// SPDX-License-Identifier: MIT OR Apache-2.0

// The transport crate intentionally has no wasm_lite dev-dependency. The same
// encoder is exercised in-browser by logwise_integration_tests/wasm_wire.rs.
#![cfg(not(target_arch = "wasm32"))]

use logwise::{
    Class, ContextToken, Detail, EventRef, FieldMetadata, FieldRef, Kind, Location, Metadata,
    Privacy, Severity, ValueRef,
};
use logwise_runtime_wasm::{ABI_VERSION, Envelope, Identity, encode_envelope};

static FIELDS: &[FieldMetadata] = &[
    FieldMetadata::new("active", Privacy::SupportSafe, Detail::Core),
    FieldMetadata::new("count", Privacy::LocalOnly, Detail::Detail),
    FieldMetadata::new("label", Privacy::SupportSafe, Detail::Core),
    FieldMetadata::new("secret", Privacy::Secret, Detail::Core),
];
static METADATA: Metadata = Metadata {
    event_name: "golden.event",
    package: "fixture",
    target: "guest",
    module: "golden",
    domain: None,
    severity: Severity::Warn,
    class: Class::Forensic,
    kind: Kind::Event,
    location: Some(Location::new("golden.rs", 17, 4)),
    fields: FIELDS,
};

#[test]
fn golden_vector_matches_minimal_embedder() {
    let fields = [
        Some(FieldRef::new(&FIELDS[0], ValueRef::Bool(true))),
        Some(FieldRef::new(&FIELDS[1], ValueRef::U64(99))),
        Some(FieldRef::new(&FIELDS[2], ValueRef::Str("héllo"))),
        Some(FieldRef::new(&FIELDS[3], ValueRef::Str("never encode"))),
    ];
    let event = EventRef::structured(&METADATA, ContextToken::from_parts(11, 12), &fields);
    let links = [ContextToken::from_parts(21, 22)];
    let envelope = Envelope {
        event,
        sequence: 42,
        dropped_before: 3,
        truncated_before: 9,
        omitted_fields: 2,
        links: &links,
        identity: Identity {
            worker: 7,
            test: Some("case"),
        },
    };
    let mut output = [0_u8; 512];
    let mut scratch = [0_u8; 32];
    let encoded = encode_envelope(envelope, 4, &mut output, &mut scratch).unwrap();
    assert_eq!(encoded.truncated_values, 1);

    let hex = encoded
        .bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(hex, include_str!("../golden/logwise_v1_event.hex").trim());

    let mut host = MinimalHost::new(encoded.bytes);
    assert_eq!(host.take(4), b"LW1\0");
    assert_eq!(host.u16(), ABI_VERSION);
    assert_eq!(host.u16(), 1, "truncation flag");
    assert_eq!(host.u32() as usize, encoded.bytes.len());
    assert_eq!(host.u64(), 42);
    assert_eq!(host.u64(), 3);
    assert_eq!(host.u64(), 9);
    assert_eq!(host.u64(), 7);
    assert_eq!(host.u64(), 11);
    assert_eq!(host.u64(), 12);
    assert_eq!(host.u16(), 1);
    assert_eq!(host.u64(), 21);
    assert_eq!(host.u64(), 22);
    assert_eq!(host.u8(), Severity::Warn as u8);
    assert_eq!(host.u8(), Class::Forensic as u8);
    assert_eq!(host.u8(), Kind::Event as u8);
    assert_eq!(host.text(), "golden.event");
    assert_eq!(host.text(), "fixture");
    assert_eq!(host.text(), "guest");
    assert_eq!(host.text(), "golden");
    assert_eq!(host.u8(), 0, "no domain");
    assert_eq!(host.u8(), 1, "test identity present");
    assert_eq!(host.text(), "case");
    assert_eq!(host.u8(), 1, "location present");
    assert_eq!(host.text(), "golden.rs");
    assert_eq!(host.u32(), 17);
    assert_eq!(host.u32(), 4);
    assert_eq!(host.u32(), 3, "two declared plus one secret omission");
    assert_eq!(host.u16(), 3);

    assert_eq!(host.text(), "active");
    assert_eq!(host.u8(), Privacy::SupportSafe as u8);
    assert_eq!(host.u8(), Detail::Core as u8);
    assert_eq!(host.u8(), 1, "bool tag");
    assert_eq!(host.u8(), 1);

    assert_eq!(host.text(), "count");
    assert_eq!(host.u8(), Privacy::LocalOnly as u8);
    assert_eq!(host.u8(), Detail::Detail as u8);
    assert_eq!(host.u8(), 3, "u64 tag");
    assert_eq!(host.u64(), 99);

    assert_eq!(host.text(), "label");
    assert_eq!(host.u8(), Privacy::SupportSafe as u8);
    assert_eq!(host.u8(), Detail::Core as u8);
    assert_eq!(host.u8(), 5, "string tag");
    assert_eq!(host.text(), "hél");
    assert_eq!(host.u8(), 0, "no message");
    assert!(host.remaining().is_empty());
}

struct MinimalHost<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> MinimalHost<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(&mut self, count: usize) -> &'a [u8] {
        let start = self.position;
        self.position += count;
        &self.bytes[start..self.position]
    }

    fn u8(&mut self) -> u8 {
        self.take(1)[0]
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.take(2).try_into().unwrap())
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }

    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take(8).try_into().unwrap())
    }

    fn text(&mut self) -> &'a str {
        let length = self.u16() as usize;
        std::str::from_utf8(self.take(length)).unwrap()
    }

    fn remaining(&self) -> &'a [u8] {
        &self.bytes[self.position..]
    }
}
