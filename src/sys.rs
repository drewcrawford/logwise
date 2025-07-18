//SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(target_arch="wasm32")]
pub use web_time::{Instant,Duration};
#[cfg(not(target_arch="wasm32"))]
pub use std::time::{Instant,Duration};