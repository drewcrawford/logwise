// SPDX-License-Identifier: MIT OR Apache-2.0

#![no_std]
#![forbid(unsafe_code)]

//! Structured wasm transport for [`logwise`].
//!
//! The reserved `logwise_v1` host ABI is implemented in the dedicated wasm
//! transport issue. This package intentionally has no dependency on an
//! executor, `wasm_lite`, `wasm-bindgen`, or `web-sys`.
