// SPDX-License-Identifier: MIT OR Apache-2.0

//! Platform-specific time types for cross-platform compatibility.
//!
//! This module provides re-exports of `Duration` and `Instant` that work
//! across native and WebAssembly targets. Both come from `wasm_lite_std::time`,
//! which re-exports `std::time` verbatim on native and substitutes
//! `performance.now()`-backed equivalents on wasm32.
//!
//! Use `crate::sys::Instant` throughout the crate rather than
//! `std::time::Instant`, which does not work on wasm32.
//!
//! # Public API
//!
//! The [`Duration`] type is re-exported at the crate root for use with
//! the [`heartbeat`](crate::heartbeat) function and
//! [`InMemoryLogger::periodic_drain_to_console`](crate::InMemoryLogger::periodic_drain_to_console).

pub use wasm_lite_std::time::{Duration, Instant};
