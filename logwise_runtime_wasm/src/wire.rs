// SPDX-License-Identifier: MIT OR Apache-2.0

use core::fmt::{self, Write};
use core::sync::atomic::{AtomicU64, Ordering};

use logwise::{ContextToken, EventRef, Privacy, ValueRef};

pub const ABI_VERSION: u16 = 1;
const MAGIC: &[u8; 4] = b"LW1\0";
const FLAG_TRUNCATED: u16 = 1;

/// Host/test identity attached by the platform integration above the facade.
#[derive(Clone, Copy, Debug, Default)]
pub struct Identity<'a> {
    pub worker: u64,
    pub test: Option<&'a str>,
}

/// A projected event plus transport-owned ordering and loss metadata.
#[derive(Clone, Copy)]
pub struct Envelope<'a> {
    pub event: EventRef<'a>,
    pub sequence: u64,
    pub dropped_before: u64,
    pub truncated_before: u64,
    pub omitted_fields: u32,
    pub links: &'a [ContextToken],
    pub identity: Identity<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncodeError {
    BufferTooSmall,
    SchemaStringTooLong,
    TooManyFields,
    TooManyLinks,
}

/// A complete versioned envelope borrowed from the caller's output buffer.
pub struct EncodedEnvelope<'a> {
    pub bytes: &'a [u8],
    pub truncated_values: u32,
}

/// Result returned by the reserved host import.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostStatus {
    Accepted,
    Unavailable,
    VersionMismatch,
    Dropped,
    HostError(i32),
}

/// Incremental structured transport state. Each call is independently framed,
/// so a host can mirror accepted envelopes before a wasm scheduler stalls.
pub struct Transport {
    next_sequence: AtomicU64,
    dropped: AtomicU64,
    truncated: AtomicU64,
    max_value_bytes: usize,
}

impl Transport {
    pub const fn new(max_value_bytes: usize) -> Self {
        Self {
            next_sequence: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            truncated: AtomicU64::new(0),
            max_value_bytes,
        }
    }

    pub fn encode<'buffer>(
        &self,
        event: EventRef<'_>,
        links: &[ContextToken],
        identity: Identity<'_>,
        output: &'buffer mut [u8],
        scratch: &mut [u8],
    ) -> Result<EncodedEnvelope<'buffer>, EncodeError> {
        let sequence = self.next_sequence.fetch_add(1, Ordering::AcqRel) + 1;
        let envelope = Envelope {
            event,
            sequence,
            dropped_before: self.dropped.load(Ordering::Acquire),
            truncated_before: self.truncated.load(Ordering::Acquire),
            omitted_fields: 0,
            links,
            identity,
        };
        match encode_envelope(envelope, self.max_value_bytes, output, scratch) {
            Ok(encoded) => {
                self.truncated
                    .fetch_add(encoded.truncated_values as u64, Ordering::Relaxed);
                Ok(encoded)
            }
            Err(error) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                Err(error)
            }
        }
    }

    pub fn emit(
        &self,
        event: EventRef<'_>,
        links: &[ContextToken],
        identity: Identity<'_>,
        output: &mut [u8],
        scratch: &mut [u8],
    ) -> Result<HostStatus, EncodeError> {
        let encoded = self.encode(event, links, identity, output, scratch)?;
        let status = host_emit(encoded.bytes);
        if status != HostStatus::Accepted {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
        Ok(status)
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn truncated(&self) -> u64 {
        self.truncated.load(Ordering::Relaxed)
    }
}

/// Encodes the stable `logwise_v1` golden wire format without allocation.
/// Secret fields are omitted defensively even if a caller bypasses runtime
/// projection. Dynamic strings are truncated at UTF-8 boundaries.
pub fn encode_envelope<'buffer>(
    envelope: Envelope<'_>,
    max_value_bytes: usize,
    output: &'buffer mut [u8],
    scratch: &mut [u8],
) -> Result<EncodedEnvelope<'buffer>, EncodeError> {
    let mut encoder = Encoder {
        output,
        position: 0,
        max_value_bytes: max_value_bytes.min(u16::MAX as usize),
        scratch,
        truncated_values: 0,
    };
    encoder.bytes(MAGIC)?;
    encoder.u16(ABI_VERSION)?;
    let flags_at = encoder.reserve_u16()?;
    let length_at = encoder.reserve_u32()?;
    encoder.u64(envelope.sequence)?;
    encoder.u64(envelope.dropped_before)?;
    encoder.u64(envelope.truncated_before)?;
    encoder.u64(envelope.identity.worker)?;
    let (context, context_flags) = envelope.event.context.into_parts();
    encoder.u64(context)?;
    encoder.u64(context_flags)?;
    let link_count = u16::try_from(envelope.links.len()).map_err(|_| EncodeError::TooManyLinks)?;
    encoder.u16(link_count)?;
    for link in envelope.links {
        let (id, flags) = link.into_parts();
        encoder.u64(id)?;
        encoder.u64(flags)?;
    }

    let metadata = envelope.event.metadata;
    encoder.u8(metadata.severity as u8)?;
    encoder.u8(metadata.class as u8)?;
    encoder.u8(metadata.kind as u8)?;
    encoder.schema(metadata.event_name)?;
    encoder.schema(metadata.package)?;
    encoder.schema(metadata.target)?;
    encoder.schema(metadata.module)?;
    encoder.optional_schema(metadata.domain.map(|domain| domain.name))?;
    encoder.optional_dynamic(envelope.identity.test)?;
    match metadata.location {
        Some(location) => {
            encoder.u8(1)?;
            encoder.schema(location.file)?;
            encoder.u32(location.line)?;
            encoder.u32(location.column)?;
        }
        None => encoder.u8(0)?,
    }

    let retained_fields = envelope
        .event
        .fields
        .iter()
        .flatten()
        .filter(|field| field.metadata.privacy != Privacy::Secret)
        .count();
    let field_count = u16::try_from(retained_fields).map_err(|_| EncodeError::TooManyFields)?;
    let implicit_omissions = metadata.fields.len().saturating_sub(retained_fields);
    let implicit_omissions = u32::try_from(implicit_omissions).unwrap_or(u32::MAX);
    encoder.u32(envelope.omitted_fields.saturating_add(implicit_omissions))?;
    encoder.u16(field_count)?;
    for field in envelope
        .event
        .fields
        .iter()
        .flatten()
        .filter(|field| field.metadata.privacy != Privacy::Secret)
    {
        encoder.schema(field.metadata.name)?;
        encoder.u8(field.metadata.privacy as u8)?;
        encoder.u8(field.metadata.detail as u8)?;
        encoder.value(field.value)?;
    }
    match envelope.event.message {
        Some(message) => {
            encoder.u8(1)?;
            encoder.formatted(message)?;
        }
        None => encoder.u8(0)?,
    }

    let flags = u16::from(encoder.truncated_values != 0) * FLAG_TRUNCATED;
    encoder.patch_u16(flags_at, flags);
    let length = u32::try_from(encoder.position).map_err(|_| EncodeError::BufferTooSmall)?;
    encoder.patch_u32(length_at, length);
    let position = encoder.position;
    let truncated_values = encoder.truncated_values;
    Ok(EncodedEnvelope {
        bytes: &output[..position],
        truncated_values,
    })
}

struct Encoder<'a> {
    output: &'a mut [u8],
    position: usize,
    max_value_bytes: usize,
    scratch: &'a mut [u8],
    truncated_values: u32,
}

impl Encoder<'_> {
    fn reserve(&mut self, count: usize) -> Result<usize, EncodeError> {
        let start = self.position;
        self.position = self
            .position
            .checked_add(count)
            .filter(|end| *end <= self.output.len())
            .ok_or(EncodeError::BufferTooSmall)?;
        Ok(start)
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        let start = self.reserve(bytes.len())?;
        self.output[start..self.position].copy_from_slice(bytes);
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), EncodeError> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), EncodeError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), EncodeError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), EncodeError> {
        self.bytes(&value.to_le_bytes())
    }

    fn reserve_u16(&mut self) -> Result<usize, EncodeError> {
        let position = self.reserve(2)?;
        self.patch_u16(position, 0);
        Ok(position)
    }

    fn reserve_u32(&mut self) -> Result<usize, EncodeError> {
        let position = self.reserve(4)?;
        self.patch_u32(position, 0);
        Ok(position)
    }

    fn patch_u16(&mut self, position: usize, value: u16) {
        self.output[position..position + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn patch_u32(&mut self, position: usize, value: u32) {
        self.output[position..position + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn schema(&mut self, value: &str) -> Result<(), EncodeError> {
        let length = u16::try_from(value.len()).map_err(|_| EncodeError::SchemaStringTooLong)?;
        self.u16(length)?;
        self.bytes(value.as_bytes())
    }

    fn optional_schema(&mut self, value: Option<&str>) -> Result<(), EncodeError> {
        match value {
            Some(value) => {
                self.u8(1)?;
                self.schema(value)
            }
            None => self.u8(0),
        }
    }

    fn optional_dynamic(&mut self, value: Option<&str>) -> Result<(), EncodeError> {
        match value {
            Some(value) => {
                self.u8(1)?;
                self.dynamic(value, false)
            }
            None => self.u8(0),
        }
    }

    fn dynamic(&mut self, value: &str, was_truncated: bool) -> Result<(), EncodeError> {
        let mut length = value.len().min(self.max_value_bytes);
        while length > 0 && !value.is_char_boundary(length) {
            length -= 1;
        }
        if was_truncated || length != value.len() {
            self.truncated_values = self.truncated_values.saturating_add(1);
        }
        self.u16(length as u16)?;
        self.bytes(&value.as_bytes()[..length])
    }

    fn formatted(&mut self, arguments: fmt::Arguments<'_>) -> Result<(), EncodeError> {
        let (length, truncated) = {
            let mut writer = ScratchWriter {
                output: self.scratch,
                position: 0,
                truncated: false,
            };
            let _ = fmt::write(&mut writer, arguments);
            (writer.position, writer.truncated)
        };
        let value = core::str::from_utf8(&self.scratch[..length])
            .expect("ScratchWriter preserves UTF-8 boundaries");
        let mut output_length = value.len().min(self.max_value_bytes);
        while output_length > 0 && !value.is_char_boundary(output_length) {
            output_length -= 1;
        }
        if truncated || output_length != value.len() {
            self.truncated_values = self.truncated_values.saturating_add(1);
        }
        let start = self.reserve(2 + output_length)?;
        self.output[start..start + 2].copy_from_slice(&(output_length as u16).to_le_bytes());
        self.output[start + 2..start + 2 + output_length]
            .copy_from_slice(&self.scratch[..output_length]);
        Ok(())
    }

    fn value(&mut self, value: ValueRef<'_>) -> Result<(), EncodeError> {
        match value {
            ValueRef::Bool(value) => {
                self.u8(1)?;
                self.u8(u8::from(value))
            }
            ValueRef::I64(value) => {
                self.u8(2)?;
                self.u64(value as u64)
            }
            ValueRef::U64(value) => {
                self.u8(3)?;
                self.u64(value)
            }
            ValueRef::F64(value) => {
                self.u8(4)?;
                self.u64(value.to_bits())
            }
            ValueRef::Str(value) => {
                self.u8(5)?;
                self.dynamic(value, false)
            }
            ValueRef::Debug(value) => {
                self.u8(6)?;
                self.formatted(format_args!("{value:?}"))
            }
            ValueRef::Display(value) => {
                self.u8(7)?;
                self.formatted(format_args!("{value}"))
            }
        }
    }
}

struct ScratchWriter<'a> {
    output: &'a mut [u8],
    position: usize,
    truncated: bool,
}

impl Write for ScratchWriter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let available = self.output.len().saturating_sub(self.position);
        let mut length = value.len().min(available);
        while length > 0 && !value.is_char_boundary(length) {
            length -= 1;
        }
        self.output[self.position..self.position + length]
            .copy_from_slice(&value.as_bytes()[..length]);
        self.position += length;
        if length != value.len() {
            self.truncated = true;
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

#[cfg(all(target_arch = "wasm32", feature = "host-abi"))]
#[allow(unsafe_code)]
fn host_emit(bytes: &[u8]) -> HostStatus {
    #[link(wasm_import_module = "logwise_v1")]
    unsafe extern "C" {
        #[link_name = "emit"]
        fn emit(pointer: u32, length: u32) -> i32;
    }

    // SAFETY: wasm32 pointers and slice lengths fit u32. The host may borrow
    // the envelope only for this call and reports disposition synchronously.
    let result = unsafe { emit(bytes.as_ptr() as u32, bytes.len() as u32) };
    decode_status(result)
}

#[cfg(not(all(target_arch = "wasm32", feature = "host-abi")))]
fn host_emit(_bytes: &[u8]) -> HostStatus {
    HostStatus::Unavailable
}

#[cfg(all(target_arch = "wasm32", feature = "host-abi"))]
fn decode_status(status: i32) -> HostStatus {
    match status {
        0 => HostStatus::Accepted,
        1 => HostStatus::Unavailable,
        2 => HostStatus::VersionMismatch,
        3 => HostStatus::Dropped,
        other => HostStatus::HostError(other),
    }
}
