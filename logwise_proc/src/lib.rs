// SPDX-License-Identifier: MIT OR Apache-2.0
//SPDX-License-Identifier: MIT OR Apache-2.0

//! # Logwise Procedural Macros
//!
//! This crate provides procedural macros for the logwise logging library, generating efficient
//! structured logging code at compile time. The macros transform format strings with key-value
//! pairs into optimized logging calls that use the `PrivateFormatter` system.
//!
//! ## Architecture
//!
//! Each logging macro follows a consistent three-phase pattern:
//! 1. **Pre-phase**: Creates a `LogRecord` using `*_pre()` functions with location metadata
//! 2. **Format phase**: Uses `PrivateFormatter` to write structured data via `lformat_impl`
//! 3. **Post-phase**: Completes logging using `*_post()` functions (sync or async variants)
//!
//! ## Log Levels and Build Configuration
//!
//! The macros respect logwise's opinionated logging levels:
//! - `trace_*`: Debug builds only, per-thread activation via `Context::currently_tracing()`
//! - `debuginternal_*`: Debug builds only, requires `declare_logging_domain!()` at crate root
//! - `info_*`: Debug builds only, for supporting downstream crates
//! - `warn_*`, `error_*`, `perfwarn_*`: Available in all builds
//!
//! ## Usage Example
//!
//! ```rust
//! use logwise_proc::*;
//!
//! // This macro call:
//! // logwise::info_sync!("User {name} has {count} items", name="alice", count=42);
//!
//! // Expands to approximately:
//! // {
//! //     let mut record = logwise::hidden::info_sync_pre(file!(), line!(), column!());
//! //     let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
//! //     formatter.write_literal("User ");
//! //     formatter.write_val("alice");
//! //     formatter.write_literal(" has ");
//! //     formatter.write_val(42);
//! //     formatter.write_literal(" items");
//! //     logwise::hidden::info_sync_post(record);
//! // }
//! ```
//!
//! ## Key-Value Parsing
//!
//! Format strings support embedded key-value pairs:
//! - Keys are extracted from `{key}` placeholders in the format string
//! - Values are provided as `key=value` parameters after the format string
//! - The parser handles complex Rust expressions as values, including method calls and literals
//!
//! ## Privacy Integration
//!
//! These macros integrate with logwise's privacy system via the `Loggable` trait.
//! Values are processed through `formatter.write_val()` which respects privacy constraints.

use proc_macro::TokenStream;

pub(crate) mod parser;

mod debug_internal;
mod error;
mod info;
mod lformat;
mod mandatory;
mod perfwarn;
mod profile;
mod profile_attr;
mod trace;
mod warn;

/// Low-level macro for generating formatter calls from format strings.
///
/// See the internal `lformat` module implementation for details.
#[proc_macro]
pub fn lformat(input: TokenStream) -> TokenStream {
    lformat::lformat_macro(input)
}

/// Synchronous trace-level logging (debug builds only, per-thread activation).
///
/// Active when `Context::currently_tracing()` is true. Compiled out in release builds.
///
/// ```
/// logwise::trace_sync!("Processing {value}", value=42);
/// ```
#[proc_macro]
pub fn trace_sync(input: TokenStream) -> TokenStream {
    trace::trace_sync_impl(input)
}

/// Asynchronous trace-level logging (debug builds only, per-thread activation).
///
/// Async variant of `trace_sync!`. Compiled out in release builds.
///
/// ```
/// async fn example() {
///     logwise::trace_async!("Processing {size} bytes", size=42);
/// }
/// ```
#[proc_macro]
pub fn trace_async(input: TokenStream) -> TokenStream {
    trace::trace_async_impl(input)
}

/// Synchronous debug-internal logging (debug builds only).
///
/// Requires `declare_logging_domain!()` at crate root. Compiled out in release builds.
///
/// ```
/// logwise::declare_logging_domain!();
/// fn main() {
///     logwise::debuginternal_sync!("Debug: {val}", val=42);
/// }
/// ```
#[proc_macro]
pub fn debuginternal_sync(input: TokenStream) -> TokenStream {
    debug_internal::debuginternal_sync_impl(input)
}

/// Asynchronous debug-internal logging (debug builds only).
///
/// Async variant of `debuginternal_sync!`. Requires `declare_logging_domain!()` at crate root.
///
/// ```
/// logwise::declare_logging_domain!();
/// async fn example() {
///     logwise::debuginternal_async!("Starting {id}", id="task_123");
/// }
/// ```
#[proc_macro]
pub fn debuginternal_async(input: TokenStream) -> TokenStream {
    debug_internal::debuginternal_async_impl(input)
}

/// Synchronous info-level logging (debug builds only).
///
/// For important operational information. Compiled out in release builds.
///
/// ```
/// logwise::info_sync!("Processing {count} items", count=42);
/// ```
#[proc_macro]
pub fn info_sync(input: TokenStream) -> TokenStream {
    info::info_sync_impl(input)
}

/// Asynchronous info-level logging (debug builds only).
///
/// Async variant of `info_sync!`. Compiled out in release builds.
///
/// ```
/// async fn example() {
///     logwise::info_async!("Connected to {host}", host="localhost");
/// }
/// ```
#[proc_macro]
pub fn info_async(input: TokenStream) -> TokenStream {
    info::info_async_impl(input)
}

/// Synchronous warning-level logging (all builds).
///
/// For suspicious conditions that warrant attention. Active in release builds.
///
/// ```
/// logwise::warn_sync!("Large payload: {size} bytes", size=1024);
/// ```
#[proc_macro]
pub fn warn_sync(input: TokenStream) -> TokenStream {
    warn::warn_sync_impl(input)
}

/// Begin a performance warning interval (all builds).
///
/// Returns a guard that warns on drop if operation takes too long. Active in release builds.
///
/// ```
/// let interval = logwise::perfwarn_begin!("Database query");
/// // ... operation ...
/// drop(interval);
/// ```
#[proc_macro]
pub fn perfwarn_begin(input: TokenStream) -> TokenStream {
    perfwarn::perfwarn_begin_impl(input)
}

/// Block-scoped performance warning interval (all builds).
///
/// Wraps a code block with performance monitoring. Preserves block's return value.
///
/// ```
/// # fn expensive() -> i32 { 42 }
/// let result = logwise::perfwarn!("Loading users", {
///     expensive()
/// });
/// ```
#[proc_macro]
pub fn perfwarn(input: TokenStream) -> TokenStream {
    perfwarn::perfwarn_impl(input)
}

/// Conditional performance warning interval.
///
/// Only logs if duration exceeds the specified threshold.
///
/// ```
/// # use std::time::Duration;
/// let threshold = Duration::from_millis(100);
/// let interval = logwise::perfwarn_begin_if!(threshold, "operation {p}", p=42);
/// drop(interval);
/// ```
#[proc_macro]
pub fn perfwarn_begin_if(input: TokenStream) -> TokenStream {
    perfwarn::perfwarn_begin_if_impl(input)
}

/// Synchronous error-level logging (all builds).
///
/// For actual error conditions in Result error paths. Active in release builds.
///
/// ```
/// logwise::error_sync!("Failed to read file: {error}", error="not found");
/// ```
#[proc_macro]
pub fn error_sync(input: TokenStream) -> TokenStream {
    error::error_sync_impl(input)
}

/// Asynchronous error-level logging (all builds).
///
/// Async variant of `error_sync!`. Active in release builds.
///
/// ```
/// async fn example() {
///     logwise::error_async!("API failed: {error}", error="timeout");
/// }
/// ```
#[proc_macro]
pub fn error_async(input: TokenStream) -> TokenStream {
    error::error_async_impl(input)
}

/// Synchronous mandatory-level logging (all builds).
///
/// Always enabled for temporary printf-style debugging. Remove before committing.
///
/// ```
/// logwise::mandatory_sync!("Debug value: {val}", val=42);
/// ```
#[proc_macro]
pub fn mandatory_sync(input: TokenStream) -> TokenStream {
    mandatory::mandatory_sync_impl(input)
}

/// Asynchronous mandatory-level logging (all builds).
///
/// Async variant of `mandatory_sync!`. Remove before committing.
///
/// ```
/// async fn example() {
///     logwise::mandatory_async!("Debug: {val}", val=42);
/// }
/// ```
#[proc_macro]
pub fn mandatory_async(input: TokenStream) -> TokenStream {
    mandatory::mandatory_async_impl(input)
}

/// Synchronous profile-level logging (all builds).
///
/// For temporary profiling and performance investigation. Remove before committing.
///
/// ```
/// logwise::profile_sync!("Operation took {ms} ms", ms=100);
/// ```
#[proc_macro]
pub fn profile_sync(input: TokenStream) -> TokenStream {
    profile::profile_sync_impl(input)
}

/// Asynchronous profile-level logging (all builds).
///
/// Async variant of `profile_sync!`. Remove before committing.
///
/// ```
/// async fn example() {
///     logwise::profile_async!("Async timing: {ms} ms", ms=50);
/// }
/// ```
#[proc_macro]
pub fn profile_async(input: TokenStream) -> TokenStream {
    profile::profile_async_impl(input)
}

/// Begin a profile interval (all builds).
///
/// Logs BEGIN when created and END with duration when dropped. Each interval has a unique ID.
///
/// ```
/// let interval = logwise::profile_begin!("database_query");
/// // ... operation ...
/// drop(interval);
/// ```
#[proc_macro]
pub fn profile_begin(input: TokenStream) -> TokenStream {
    profile::profile_begin_impl(input)
}

/// Attribute macro to automatically profile a function's execution time.
///
/// Wraps function body to log BEGIN on entry and END with duration on return.
///
/// ```rust
/// #[logwise::profile]
/// fn expensive_operation() -> i32 {
///     (0..1000).sum()
/// }
/// ```
#[proc_macro_attribute]
pub fn profile(attr: TokenStream, item: TokenStream) -> TokenStream {
    profile_attr::profile_attr_impl(attr, item)
}
