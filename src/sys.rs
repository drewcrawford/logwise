// SPDX-License-Identifier: MIT OR Apache-2.0

//! Platform-specific time types for cross-platform compatibility.
//!
//! This module provides re-exports of `Duration` and `Instant` that work
//! across native and WebAssembly targets. On native platforms, these come
//! from `std::time`, while on WASM they come from `web_time`.
//!
//! # Public API
//!
//! The [`Duration`] type is re-exported at the crate root for use with
//! the [`heartbeat`](crate::heartbeat) function and
//! [`InMemoryLogger::periodic_drain_to_console`](crate::InMemoryLogger::periodic_drain_to_console).

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::{Duration, Instant};
#[cfg(target_arch = "wasm32")]
pub use web_time::{Duration, Instant};
