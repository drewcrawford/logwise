// SPDX-License-Identifier: MIT OR Apache-2.0

#![no_std]
#![deny(unsafe_code)]

//! Structured wasm transport for [`logwise`].
//!
//! The reserved `logwise_v1` host ABI is implemented in the dedicated wasm
//! transport issue. This package intentionally has no dependency on an
//! executor, `wasm_lite`, `wasm-bindgen`, or `web-sys`.

mod wire;

pub use wire::{
    ABI_VERSION, EncodeError, EncodedEnvelope, Envelope, HostStatus, Identity, Transport,
    encode_envelope,
};

/// Origin of text intercepted by a JavaScript console monkeypatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsoleOrigin {
    Log,
    Debug,
    Info,
    Warn,
    Error,
}

impl ConsoleOrigin {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Log => "js.console.log",
            Self::Debug => "js.console.debug",
            Self::Info => "js.console.info",
            Self::Warn => "js.console.warn",
            Self::Error => "js.console.error",
        }
    }
}

/// Imports already-captured legacy console text as an opaque local-only event.
///
/// This does not install or promise a console monkeypatch. Hosts such as
/// `wasm_lite` keep owning that best-effort interception and call this adapter;
/// first-party structured events use the reserved `logwise_v1` transport.
pub fn ingest_console(origin: ConsoleOrigin, text: &str) {
    logwise::__logwise_structured!(
        None;
        diagnostic,
        debug,
        AdHocText,
        "foreign.text",
        origin = local(origin.as_str()),
        text = local(text),
    );
}
