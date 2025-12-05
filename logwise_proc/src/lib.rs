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

use proc_macro::{TokenStream, TokenTree};
use std::collections::VecDeque;

pub(crate) mod parser;
use parser::lformat_impl;

mod debug_internal;
mod error;
mod info;
mod mandatory;
mod perfwarn;
mod profile;
mod trace;
mod warn;

/// Low-level macro for generating formatter calls from format strings.
///
/// This macro is used internally by the logging macros to transform format strings
/// with key-value pairs into sequences of `write_literal()` and `write_val()` calls
/// on a formatter object. It's exposed publicly for advanced use cases but most users
/// should use the higher-level logging macros instead.
///
/// # Syntax
/// ```
/// # struct Formatter;
/// # impl Formatter {
/// #    fn write_literal(&self, s: &str) {}
/// #    fn write_val(&self, s: u8) {}
/// # }
/// # let format_ident = Formatter;
/// # use logwise_proc::lformat;
/// lformat!(format_ident, "format string with {key1} {key2}", key1=2, key2=3);
/// ```
///
/// # Arguments
/// * `formatter_ident` - The name of the formatter variable to call methods on
/// * `format_string` - A string literal with `{key}` placeholders
/// * `key=value` pairs - Values to substitute for placeholders in the format string
///
/// # Generated Code
/// The macro generates a sequence of method calls on the formatter:
/// - `formatter.write_literal("text")` for literal text portions
/// - `formatter.write_val(expression)` for placeholder values
///
/// # Examples
///
/// Basic usage with a mock formatter:
/// ```
/// # struct Logger;
/// # impl Logger {
/// #   fn write_literal(&self, s: &str) {}
/// #   fn write_val(&self, s: u8) {}
/// # }
/// # let logger = Logger;
/// use logwise_proc::lformat;
/// lformat!(logger,"Hello, {world}!", world=23);
/// ```
///
/// This expands to approximately:
/// ```ignore
/// # // ignore because: This shows macro expansion output, not actual runnable code
/// logger.write_literal("Hello, ");
/// logger.write_val(23);
/// logger.write_literal("!");
/// ```
///
/// Complex expressions as values:
/// ```
/// # struct Logger;
/// # impl Logger {
/// #   fn write_literal(&self, s: &str) {}
/// #   fn write_val<A>(&self, s: A) {}
/// # }
/// # let logger = Logger;
/// # use logwise_proc::lformat;
/// // This works with any expression
/// lformat!(logger, "User {a} has {b} items",
///          a = "hi",
///          b = 3);
/// ```
///
/// # Error Cases
///
/// Missing formatter identifier:
/// ```compile_fail
/// use logwise_proc::lformat;
/// lformat!(23);
/// ```
///
/// Missing key in format string:
/// ```compile_fail
/// # struct Logger;
/// # impl Logger {
/// #   fn write_literal(&self, s: &str) {}
/// #   fn write_val(&self, s: u8) {}
/// # }
/// # let logger = Logger;
/// use logwise_proc::lformat;
/// lformat!(logger, "Hello {missing}!", provided=123);
/// ```
#[proc_macro]
pub fn lformat(input: TokenStream) -> TokenStream {
    let mut collect: VecDeque<_> = input.into_iter().collect();

    //get logger ident
    let logger_ident = match collect.pop_front() {
        Some(TokenTree::Ident(i)) => i,
        _ => {
            return r#"compile_error!("lformat!() must be called with a logger ident")"#
                .parse()
                .unwrap();
        }
    };
    //eat comma
    match collect.pop_front() {
        Some(TokenTree::Punct(p)) => {
            if p.as_char() != ',' {
                return r#"compile_error!("Expected ','")"#.parse().unwrap();
            }
        }
        _ => {
            return r#"compile_error!("Expected ','")"#.parse().unwrap();
        }
    }

    let o = lformat_impl(&mut collect, logger_ident.to_string());
    o.output
}

/// Generates synchronous trace-level logging code (debug builds only).
///
/// This macro creates trace-level log entries that are only active in debug builds
/// and when `Context::currently_tracing()` returns true. Trace logging is designed
/// for detailed debugging information that can be activated per-thread.
///
/// # Syntax
/// ```
/// logwise::trace_sync!("format string {key}", key="value");
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Active when `Context::currently_tracing()` is true
/// - **Release builds**: Completely compiled out (no runtime cost)
///
/// # Generated Code Pattern
/// ```ignore
/// # // ignore because: This illustrates macro expansion output, not actual runnable code
/// #[cfg(debug_assertions)]
/// {
///     if logwise::context::Context::currently_tracing() {
///         let mut record = logwise::hidden::trace_sync_pre(file!(), line!(), column!());
///         let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///         // ... formatter calls ...
///         logwise::hidden::trace_sync_post(record);
///     }
/// }
/// ```
///
/// # Examples
/// ```
/// // Basic trace logging
/// logwise::trace_sync!("Entering function with param {value}", value=42);
///
/// // With complex expressions
/// logwise::trace_sync!("User {name} processing", name="john");
///
/// // With privacy-aware values
/// let sensitive_data = "secret job";
/// logwise::trace_sync!("Processing {data}", data=logwise::privacy::LogIt(sensitive_data));
/// ```
#[proc_macro]
pub fn trace_sync(input: TokenStream) -> TokenStream {
    trace::trace_sync_impl(input)
}

/// Generates asynchronous trace-level logging code (debug builds only).
///
/// This macro creates async trace-level log entries that are only active in debug builds
/// and when `Context::currently_tracing()` returns true. The async variant is suitable
/// for use in async contexts where logging might involve async operations.
///
/// # Syntax
/// ```
/// async fn example() {
///     logwise::trace_async!("format string");
/// }
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Active when `Context::currently_tracing()` is true
/// - **Release builds**: Completely compiled out (no runtime cost)
///
/// # Generated Code Pattern
/// ```ignore
/// # // ignore because: This illustrates macro expansion output, not actual runnable code
/// #[cfg(debug_assertions)]
/// {
///     if logwise::context::Context::currently_tracing() {
///         let mut record = logwise::hidden::trace_sync_pre(file!(), line!(), column!());
///         let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///         // ... formatter calls ...
///         logwise::hidden::trace_async_post(record).await;
///     }
/// }
/// ```
///
/// # Examples
/// ```
/// // In async function context
/// async fn process_data(data: &[u8]) {
///     logwise::trace_async!("Processing {size} bytes", size=data.len());
///     // ... async work ...
/// }
/// ```
#[proc_macro]
pub fn trace_async(input: TokenStream) -> TokenStream {
    trace::trace_async_impl(input)
}

/// Generates synchronous debug-internal logging code (debug builds only).
///
/// This macro creates debug-internal log entries that are only active in debug builds.
/// It's designed for print-style debugging within the current crate and requires the
/// `declare_logging_domain!()` macro to be used at the crate root.
///
/// # Prerequisites
/// You must declare a logging domain at your crate root:
/// ```
/// logwise::declare_logging_domain!();
/// ```
///
/// # Syntax
/// ```
/// logwise::declare_logging_domain!();
/// fn main() {
/// let key = "example_value";
/// logwise::debuginternal_sync!("format string with {placeholders}", placeholders=key);
/// }
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Active when domain is internal OR `Context::currently_tracing()` is true
/// - **Release builds**: Completely compiled out (no runtime cost)
///
/// # Generated Code Pattern
/// ```ignore
/// # // ignore because: This illustrates macro expansion output, not actual runnable code
/// #[cfg(debug_assertions)]
/// {
///     let use_declare_logging_domain_macro_at_crate_root = crate::__LOGWISE_DOMAIN.is_internal();
///     if use_declare_logging_domain_macro_at_crate_root || logwise::context::Context::currently_tracing() {
///         let mut record = logwise::hidden::debuginternal_pre(file!(), line!(), column!());
///         let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///         // ... formatter calls ...
///         logwise::hidden::debuginternal_sync_post(record);
///     }
/// }
/// ```
///
/// # Examples
/// ```
/// logwise::declare_logging_domain!();
/// fn main() {
/// // In your code:
/// let current_state = 42;
/// logwise::debuginternal_sync!("Debug: value is {val}", val=current_state);
///
/// // With complex expressions
/// # use std::collections::VecDeque;
/// let mut queue = VecDeque::new();
/// queue.push_front("item");
/// logwise::debuginternal_sync!("Processing {item}", item=logwise::privacy::LogIt(&queue.front().unwrap()));
/// }
/// ```
///
/// # Error Conditions
/// If you forget to use declare_logging_domain!(), you'll get a compile error
/// about __LOGWISE_DOMAIN not being found.
#[proc_macro]
pub fn debuginternal_sync(input: TokenStream) -> TokenStream {
    debug_internal::debuginternal_sync_impl(input)
}

/// Generates asynchronous debug-internal logging code (debug builds only).
///
/// This macro creates async debug-internal log entries that are only active in debug builds.
/// Like the sync variant, it requires `declare_logging_domain!()` at the crate root and is
/// designed for print-style debugging in async contexts.
///
/// # Prerequisites
/// You must declare a logging domain at your crate root:
/// ```
/// logwise::declare_logging_domain!();
/// ```
///
/// # Syntax
/// ```
/// logwise::declare_logging_domain!(true);
/// fn main() {
///     async fn ex() {
///         let key = "example_value";
///         logwise::debuginternal_async!("format string with {placeholders}", placeholders=key);
///     }
/// }
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Active when domain is internal OR `Context::currently_tracing()` is true
/// - **Release builds**: Completely compiled out (no runtime cost)
///
/// # Examples
/// ```
/// # fn main() { }
/// # logwise::declare_logging_domain!();
/// // In async functions:
/// async fn process_async() {
///     let task_id = "task_123";
///     logwise::debuginternal_async!("Starting async work with {id}", id=task_id);
///     // ... async operations ...
/// }
/// ```
#[proc_macro]
pub fn debuginternal_async(input: TokenStream) -> TokenStream {
    debug_internal::debuginternal_async_impl(input)
}

/// Generates synchronous info-level logging code (debug builds only).
///
/// This macro creates info-level log entries designed for supporting downstream crates.
/// Info logging is intended for important operational information that helps users
/// understand what the library is doing, but is only active in debug builds.
///
/// # Syntax
/// ```
/// let key = "example_value";
/// logwise::info_sync!("format string with {placeholders}", placeholders=key);
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Completely compiled out (no runtime cost)
///
/// # Generated Code Pattern
/// ```
/// #[cfg(debug_assertions)]
/// {
///     let mut record = logwise::hidden::info_sync_pre(file!(), line!(), column!());
///     let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///     // ... formatter calls ...
///     logwise::hidden::info_sync_post(record);
/// }
/// ```
///
/// # Examples
/// ```
/// // Library operation notifications
/// # struct Pool; impl Pool { fn active_count(&self) -> usize { 42 } }
/// # let pool = Pool;
/// logwise::info_sync!("Connected to database with {connections} active", connections=pool.active_count());
///
/// // User-facing operation status
/// # let items = vec![1, 2, 3];
/// logwise::info_sync!("Processing {count} items", count=items.len());
///
/// // With privacy considerations
/// # let user_id = "user123";
/// logwise::info_sync!("User {id} authenticated", id=logwise::privacy::LogIt(user_id));
/// ```
#[proc_macro]
pub fn info_sync(input: TokenStream) -> TokenStream {
    info::info_sync_impl(input)
}

/// Generates asynchronous info-level logging code (debug builds only).
///
/// This macro creates async info-level log entries designed for supporting downstream crates
/// in async contexts. Like the sync variant, it's intended for important operational
/// information but only active in debug builds.
///
/// # Syntax
/// ```
/// async fn example() {
///     let key = "example_value";
///     logwise::info_async!("format string with {placeholders}", placeholders=key);
/// }
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Completely compiled out (no runtime cost)
///
/// # Examples
/// ```
/// struct DbConfig { host: String }
///
/// async fn example() {
///     let db_config = DbConfig { host: "localhost".to_string() };
///     logwise::info_async!("Attempting database connection to {host}", host=db_config.host);
///     // ... connection logic ...
///
///     // Progress reporting in async contexts
///     let current = 5;
///     let total_batches = 10;
///     logwise::info_async!("Processed batch {batch} of {total}", batch=current, total=total_batches);
/// }
/// ```
#[proc_macro]
pub fn info_async(input: TokenStream) -> TokenStream {
    info::info_async_impl(input)
}

/// Generates synchronous warning-level logging code (all builds).
///
/// This macro creates warning-level log entries for suspicious conditions that don't
/// necessarily indicate errors but warrant attention. Unlike debug-only macros,
/// warnings are active in both debug and release builds.
///
/// # Syntax
/// ```
/// logwise::warn_sync!("format string");
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Always active (runtime cost incurred)
///
/// # Generated Code Pattern
/// ```
/// {
///     let mut record = logwise::hidden::warn_sync_pre(file!(), line!(), column!());
///     let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///     // ... formatter calls ...
///     logwise::hidden::warn_sync_post(record);
/// }
/// ```
///
/// # Use Cases
/// - Deprecated API usage
/// - Suboptimal configuration
/// - Recoverable error conditions
/// - Security-relevant events
///
/// # Examples
/// ```
/// // Configuration warnings
/// # let old_key = "old_setting";
/// # let new_key = "new_setting";
/// logwise::warn_sync!("Using deprecated config {key}, consider {alternative}",
///                     key=old_key, alternative=new_key);
///
/// // Performance warnings
/// # let size = 1024;
/// logwise::warn_sync!("Large payload detected: {size} bytes", size=size);
///
/// // Security warnings with privacy
/// logwise::warn_sync!("Failed login attempt from {ip}",
///                     ip=logwise::privacy::IPromiseItsNotPrivate("127.0.0.1"));
/// ```
#[proc_macro]
pub fn warn_sync(input: TokenStream) -> TokenStream {
    warn::warn_sync_impl(input)
}

/// Generates code to begin a performance warning interval (all builds).
///
/// This macro starts a performance monitoring interval that will warn if an operation
/// takes longer than expected. It returns an interval guard that should be dropped
/// when the operation completes. Use `perfwarn!` for block-scoped intervals or
/// this macro for manual interval management.
///
/// # Syntax
/// ```
/// let value = 3;
/// let interval = logwise::perfwarn_begin!("operation description {param}", param=value);
/// // ... long-running operation ...
/// drop(interval); // Or let it drop automatically
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Always active (runtime cost incurred)
///
/// # Generated Code Pattern
/// ```ignore
/// # // ignore because: This illustrates macro expansion output, not actual runnable code
/// {
///     let mut record = logwise::hidden::perfwarn_begin_pre(file!(), line!(), column!());
///     let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///     // ... formatter calls ...
///     logwise::hidden::perfwarn_begin_post(record, "operation description")
/// }
/// ```
///
/// # Examples
/// ```
/// // Manual interval management
/// let interval = logwise::perfwarn_begin!("Database query");
/// // let results = db.query(&sql).await;
/// drop(interval);
///
/// // Automatic cleanup with scope
/// {
///     let _interval = logwise::perfwarn_begin!("File processing {path}", path="/example.txt");
///     // process_large_file(&path);
/// } // interval automatically dropped here
/// ```
///
/// # See Also
/// - `perfwarn!` - For block-scoped performance intervals
#[proc_macro]
pub fn perfwarn_begin(input: TokenStream) -> TokenStream {
    perfwarn::perfwarn_begin_impl(input)
}

/// Generates code for a block-scoped performance warning interval (all builds).
///
/// This macro wraps a code block with performance monitoring, automatically managing
/// the interval lifecycle. It will warn if the block execution takes longer than expected.
/// The block's return value is preserved.
///
/// # Syntax
/// ```
/// # fn expensive_operation() -> i32 { 42 }
/// let value = 123;
/// let result = logwise::perfwarn!("operation description {param}", param=value, {
///     // code block to monitor
///     expensive_operation()
/// });
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Always active (runtime cost incurred)
///
/// # Generated Code Pattern
/// ```
/// {
///     let mut record = logwise::hidden::perfwarn_begin_pre(file!(), line!(), column!());
///     let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///     // ... formatter calls ...
///     let interval = logwise::hidden::perfwarn_begin_post(record, "operation description");
///     let result = { /* user block */ };
///     drop(interval);
///     result
/// }
/// ```
///
/// # Examples
/// ```
/// # fn process<T>(_t: T) {}
/// # struct Database; impl Database { fn query(&self, _: &str) -> Vec<String> { vec![] } }
/// # let database = Database;
/// // Monitor a database operation
/// let users = logwise::perfwarn!("Loading users from {table}", table="users", {
///     database.query("SELECT * FROM users")
/// });
///
/// // Monitor file I/O
/// let path = "large_file.txt";
/// let file_size_mb = 100;
/// let content = logwise::perfwarn!("Reading large file {size} MB", size=file_size_mb, {
///     "fake file content".to_string()  // Avoiding actual file I/O in doctests
/// });
///
/// // With complex parameters  
/// let items = vec![1, 2, 3];
/// let result: Vec<()> = logwise::perfwarn!("Processing {count} items", count=items.len(), {
///     items.into_iter().map(|item| process(item)).collect()
/// });
/// ```
///
/// # See Also
/// - `perfwarn_begin!` - For manual interval management
#[proc_macro]
pub fn perfwarn(input: TokenStream) -> TokenStream {
    perfwarn::perfwarn_impl(input)
}

/// Generates code for a conditional performance warning interval.
///
/// This macro creates a performance interval that only logs if the duration exceeds
/// the specified threshold. It also contributes to task statistics, but only logs
/// the statistics if the total duration exceeds the total threshold.
///
/// # Syntax
/// ```
/// # use std::time::Duration;
/// let threshold = Duration::from_millis(100);
/// let interval = logwise::perfwarn_begin_if!(threshold, "operation name {param}", param=42);
/// // ... operation ...
/// drop(interval);
/// ```
#[proc_macro]
pub fn perfwarn_begin_if(input: TokenStream) -> TokenStream {
    perfwarn::perfwarn_begin_if_impl(input)
}

/// Generates synchronous error-level logging code (all builds).
///
/// This macro creates error-level log entries for actual error conditions that should
/// be logged in Result error paths. Error logging is active in both debug and release
/// builds since errors are always relevant.
///
/// # Syntax
/// ```
/// let key = "example_value";
/// logwise::error_sync!("format string with {placeholders}", placeholders=key);
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Always active (runtime cost incurred)
///
/// # Generated Code Pattern
/// ```
/// {
///     let mut record = logwise::hidden::error_sync_pre(file!(), line!(), column!());
///     let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///     // ... formatter calls ...
///     logwise::hidden::error_sync_post(record);
/// }
/// ```
///
/// # Use Cases
/// - I/O failures
/// - Network errors
/// - Validation failures
/// - Resource exhaustion
/// - External service errors
///
/// # Examples
/// ```
/// # fn example() -> Result<Vec<u8>, std::io::Error> {
/// # let path = std::path::Path::new("/example.txt");
/// // I/O error logging
/// let content = match std::fs::read(&path) {
///     Ok(content) => content,
///     Err(e) => {
///         logwise::error_sync!("Failed to read file {path}: {error}",
///                              path=path.to_string_lossy().to_string(), error=e.to_string());
///         return Err(e);
///     }
/// };
/// # Ok(content)
/// # }
///
/// // Database error with privacy
/// # let user_id = "user123";
/// # let db_error = "Connection timeout";
/// logwise::error_sync!("Database query failed for user {id}: {error}",
///                      id=logwise::privacy::LogIt(user_id), error=db_error);
///
/// // Network error
/// # let response_status = 500;
/// # let request_url = "https://api.example.com";
/// logwise::error_sync!("HTTP request failed: {status} {url}",
///                      status=response_status, url=request_url);
/// ```
#[proc_macro]
pub fn error_sync(input: TokenStream) -> TokenStream {
    error::error_sync_impl(input)
}

/// Generates asynchronous error-level logging code (all builds).
///
/// This macro creates async error-level log entries for actual error conditions in
/// async contexts. Like the sync variant, it's designed for Result error paths and
/// is active in both debug and release builds.
///
/// # Syntax
/// ```
/// async fn example() {
///     let key = "example_value";
///     logwise::error_async!("format string with {placeholders}", placeholders=key);
/// }
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Always active (runtime cost incurred)
///
/// # Examples
/// ```
/// async fn read_config() -> Result<String, std::io::Error> {
///     let config_path = "/config.toml";
///     
///     // Simulate config reading with error handling
///     match std::fs::read_to_string(config_path) {
///         Ok(content) => Ok(content),
///         Err(e) => {
///             logwise::error_async!("Config read failed {path}: {error}",
///                                   path=config_path, error=logwise::privacy::LogIt(&e));
///             Err(e)
///         }
///     }
/// }
/// ```
///
/// ```
/// async fn example() {
///     let retry_count = 3;
///     let last_error = "Timeout";
///     logwise::error_async!("API request failed after {attempts} attempts: {error}",
///                           attempts=retry_count, error=last_error);
/// }
/// ```
#[proc_macro]
pub fn error_async(input: TokenStream) -> TokenStream {
    error::error_async_impl(input)
}

/// Generates synchronous mandatory-level logging code (all builds).
///
/// This macro creates mandatory-level log entries that are always enabled, even in
/// release builds. Use for temporary printf-style debugging in hostile environments
/// where other log levels may be compiled out. Intended to be removed before committing.
///
/// # Syntax
/// ```
/// logwise::mandatory_sync!("Debug value: {val}", val=42);
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Always active
///
/// # Examples
/// ```
/// // Printf-style debugging
/// let x = 42;
/// logwise::mandatory_sync!("x = {val}", val=x);
///
/// // With complex expressions
/// # struct Obj { field: i32 }
/// # let obj = Obj { field: 123 };
/// logwise::mandatory_sync!("obj.field = {f}", f=obj.field);
/// ```
#[proc_macro]
pub fn mandatory_sync(input: TokenStream) -> TokenStream {
    mandatory::mandatory_sync_impl(input)
}

/// Generates asynchronous mandatory-level logging code (all builds).
///
/// This macro creates async mandatory-level log entries that are always enabled,
/// even in release builds. Use for temporary printf-style debugging in async contexts.
/// Intended to be removed before committing.
///
/// # Syntax
/// ```
/// async fn example() {
///     logwise::mandatory_async!("Debug value: {val}", val=42);
/// }
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Always active
///
/// # Examples
/// ```
/// async fn debug_async() {
///     let state = "processing";
///     logwise::mandatory_async!("Current state: {s}", s=state);
/// }
/// ```
#[proc_macro]
pub fn mandatory_async(input: TokenStream) -> TokenStream {
    mandatory::mandatory_async_impl(input)
}

/// Generates synchronous profile-level logging code (all builds).
///
/// This macro creates profile-level log entries that are always enabled, even in
/// release builds. Use for temporary profiling and performance investigation.
/// Intended to be removed before committing.
///
/// # Syntax
/// ```
/// logwise::profile_sync!("Operation took {ms} ms", ms=100);
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Always active
///
/// # Examples
/// ```
/// # use std::time::Instant;
/// let start = Instant::now();
/// // ... operation ...
/// let elapsed = start.elapsed();
/// logwise::profile_sync!("Operation completed in {ms} ms", ms=elapsed.as_millis());
/// ```
#[proc_macro]
pub fn profile_sync(input: TokenStream) -> TokenStream {
    profile::profile_sync_impl(input)
}

/// Generates asynchronous profile-level logging code (all builds).
///
/// This macro creates async profile-level log entries that are always enabled,
/// even in release builds. Use for temporary profiling in async contexts.
/// Intended to be removed before committing.
///
/// # Syntax
/// ```
/// async fn example() {
///     logwise::profile_async!("Async operation timing: {ms} ms", ms=50);
/// }
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Always active
///
/// # Examples
/// ```
/// async fn profile_async_op() {
///     let duration_ms = 150;
///     logwise::profile_async!("Async task completed in {ms} ms", ms=duration_ms);
/// }
/// ```
#[proc_macro]
pub fn profile_async(input: TokenStream) -> TokenStream {
    profile::profile_async_impl(input)
}

/// Generates code to begin a profile interval (all builds).
///
/// This macro starts a profiling interval that logs BEGIN when created and END
/// with elapsed duration when dropped. Each interval has a unique ID to correlate
/// BEGIN/END pairs, which is especially useful when profiling nested operations.
///
/// # Syntax
/// ```
/// let interval = logwise::profile_begin!("operation description {param}", param=42);
/// // ... operation to profile ...
/// drop(interval); // Or let it drop automatically
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Always active
///
/// # Log Output
/// The macro produces log messages like:
/// ```text
/// PROFILE: BEGIN [id=1] src/main.rs:10:5 [0ns] database_query
/// PROFILE: END [id=1] [elapsed: 150µs] database_query
/// ```
///
/// # Examples
/// ```
/// # fn perform_database_query() {}
/// // Profile a database operation
/// let interval = logwise::profile_begin!("database_query");
/// perform_database_query();
/// drop(interval);
///
/// // With parameters
/// # let table_name = "users";
/// let _interval = logwise::profile_begin!("query {table}", table=table_name);
/// // ... operation ...
/// // Automatically logs END when _interval goes out of scope
/// ```
///
/// # Nested Profiling
/// ```
/// # fn outer_work() {}
/// # fn inner_work() {}
/// let outer = logwise::profile_begin!("outer_operation");
/// outer_work();
/// {
///     let inner = logwise::profile_begin!("inner_operation");
///     inner_work();
///     // inner END logged here
/// }
/// // outer END logged here
/// ```
#[proc_macro]
pub fn profile_begin(input: TokenStream) -> TokenStream {
    profile::profile_begin_impl(input)
}
