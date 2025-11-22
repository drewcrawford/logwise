// SPDX-License-Identifier: MIT OR Apache-2.0

//! Core logging implementation functions for the logwise logging library.
//!
//! This module provides the low-level implementation functions that are called by the
//! procedural macros in `logwise_proc`. These functions handle the creation, formatting,
//! and dispatching of log records to global loggers.
//!
//! # Architecture
//!
//! The logging flow follows this pattern:
//! 1. A `*_pre` function creates a [`LogRecord`] with appropriate metadata
//! 2. The procedural macro uses [`PrivateFormatter`] to add the formatted message
//! 3. A `*_post` function dispatches the record to all global loggers
//!
//! # Log Levels
//!
//! Each log level has its own set of implementation functions:
//! - `trace`: Detailed debugging (debug builds only, per-thread activation)
//! - `debuginternal`: Print-style debugging (debug builds only)
//! - `info`: Supporting downstream crates (debug builds only)
//! - `perfwarn`: Performance problem logging (all builds)
//! - `warn`: Suspicious conditions (all builds)
//! - `error`: Error logging in Results (all builds)
//!
//! # Privacy
//!
//! The [`PrivateFormatter`] ensures that all logged values respect privacy constraints
//! by calling the [`Loggable::log_all`](crate::privacy::Loggable::log_all) method, which
//! includes all private information for local logging.
//!
//! # Example
//!
//! These functions are not intended to be called directly. Instead, use the macros:
//!
//! ```rust
//! # use logwise::context::Context;
//! # Context::reset("example".to_string());
//! // Use the macro, not the implementation functions
//! logwise::info_sync!("Operation completed", count=42);
//! ```

use crate::Level;
use crate::log_record::LogRecord;
use crate::privacy::Loggable;
use std::sync::atomic::AtomicBool;

/// Controls whether internal logging is enabled for a crate.
///
/// This struct manages the internal logging domain for a crate, determining
/// whether `debuginternal` log messages are displayed. The domain is typically
/// configured at compile time based on the `logwise_internal` feature flag.
///
/// # Example
///
/// ```rust
/// # use logwise::LoggingDomain;
/// // Create a domain that's enabled
/// let domain = LoggingDomain::new(true);
/// assert!(domain.is_internal());
///
/// // Create a domain that's disabled
/// let domain = LoggingDomain::new(false);
/// assert!(!domain.is_internal());
/// ```
pub struct LoggingDomain {
    is_internal: AtomicBool,
}

impl LoggingDomain {
    /// Creates a new `LoggingDomain` with the specified internal logging state.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether internal logging should be enabled for this domain
    ///
    /// # Example
    ///
    /// ```rust
    /// # use logwise::LoggingDomain;
    /// let domain = LoggingDomain::new(true);
    /// ```
    #[inline]
    pub const fn new(enabled: bool) -> Self {
        Self {
            is_internal: AtomicBool::new(enabled),
        }
    }

    /// Returns whether internal logging is enabled for this domain.
    ///
    /// This determines whether `debuginternal` log messages will be displayed
    /// when logging from this crate.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use logwise::LoggingDomain;
    /// let domain = LoggingDomain::new(true);
    /// assert!(domain.is_internal());
    /// ```
    #[inline]
    pub fn is_internal(&self) -> bool {
        self.is_internal.load(std::sync::atomic::Ordering::Relaxed)
    }
}

// ============================================================================
// Boilerplate trait implementations for LoggingDomain
// ============================================================================

impl std::fmt::Debug for LoggingDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoggingDomain")
            .field("is_internal", &self.is_internal())
            .finish()
    }
}

impl Default for LoggingDomain {
    /// Creates a new `LoggingDomain` with internal logging disabled.
    ///
    /// This is the safe default for most use cases where internal logging
    /// should be explicitly enabled rather than accidentally left on.
    fn default() -> Self {
        Self::new(false)
    }
}

impl std::fmt::Display for LoggingDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LoggingDomain(internal={})", self.is_internal())
    }
}

impl From<bool> for LoggingDomain {
    /// Creates a `LoggingDomain` from a boolean value.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether internal logging should be enabled
    ///
    /// # Example
    ///
    /// ```rust
    /// # use logwise::LoggingDomain;
    /// let domain: LoggingDomain = true.into();
    /// assert!(domain.is_internal());
    /// ```
    fn from(enabled: bool) -> Self {
        Self::new(enabled)
    }
}

/// Declares the logging domain for the current crate.
///
/// This macro sets up the internal logging configuration for a crate, determining
/// whether `debuginternal` log messages will be displayed. It must be declared at
/// the crate root (typically in `lib.rs` or `main.rs`) to enable the use of
/// `debuginternal_sync!` and `debuginternal_async!` macros.
///
/// # Two Invocation Patterns
///
/// ## 1. Default Invocation (Feature-Based)
///
/// When called without arguments, the macro automatically enables internal logging
/// based on the presence of the `logwise_internal` feature flag:
///
/// ```rust
/// # #[macro_use] extern crate logwise;
/// // At crate root (lib.rs or main.rs)
/// declare_logging_domain!();
/// ```
///
/// With this pattern, you should declare a `logwise_internal` feature in your
/// `Cargo.toml`:
///
/// ```toml
/// [features]
/// logwise_internal = []
/// ```
///
/// Then enable internal logging by running:
/// ```bash
/// cargo build --features logwise_internal
/// ```
///
/// ## 2. Explicit Invocation
///
/// You can explicitly control internal logging by passing a boolean value:
///
/// ```rust
/// # #[macro_use] extern crate logwise;
/// // Always enable internal logging for this crate
/// declare_logging_domain!(true);
/// ```
///
/// Or conditionally based on other criteria:
///
/// ```rust
/// # #[macro_use] extern crate logwise;
/// // Enable based on custom logic or configuration
/// declare_logging_domain!(cfg!(debug_assertions));
/// ```
///
/// # Important Notes
///
/// - **Required for debuginternal macros**: Without this macro at the crate root,
///   attempting to use `debuginternal_sync!` or `debuginternal_async!` will result
///   in a compile error about `__LOGWISE_DOMAIN` not being found.
///
/// - **Crate-level scope**: This affects only the current crate. Each crate using
///   logwise must declare its own logging domain.
///
/// - **Performance**: When internal logging is disabled, `debuginternal` macros
///   have minimal runtime overhead as they check the domain state early.
///
/// # Examples
///
/// ## Basic Setup with Feature Flag
///
/// `Cargo.toml`:
/// ```toml
/// [dependencies]
/// logwise = "0.1"
///
/// [features]
/// logwise_internal = []
/// ```
///
/// `src/lib.rs`:
/// ```rust
/// # #[macro_use] extern crate logwise;
/// declare_logging_domain!();
/// # fn main() { }
///
/// pub fn my_function() {
///     // This will only log when built with --features logwise_internal
///     logwise::debuginternal_sync!("Debug output: {value}", value=42);
/// }
/// ```
///
/// ## Always-Enabled Internal Logging
///
/// ```rust
/// # #[macro_use] extern crate logwise;
/// // Enable internal logging for all builds of this crate
/// declare_logging_domain!(true);
///
/// fn main() {
///     // This will always log in debug builds
///     logwise::debuginternal_sync!("Application starting");
/// }
/// ```
///
/// ## Conditional Based on Environment
///
/// ```rust
/// # #[macro_use] extern crate logwise;
/// // Enable internal logging based on an environment variable
/// declare_logging_domain!(option_env!("ENABLE_INTERNAL_LOGS").is_some());
/// ```
#[macro_export]
macro_rules! declare_logging_domain {
    () => {
        #[doc(hidden)]
        #[cfg(feature = "logwise_internal")]
        pub(crate) static __LOGWISE_DOMAIN: $crate::LoggingDomain =
            $crate::LoggingDomain::new(true);
        #[cfg(not(feature = "logwise_internal"))]
        pub(crate) static __LOGWISE_DOMAIN: $crate::LoggingDomain =
            $crate::LoggingDomain::new(false);
    };
    ($enabled:expr) => {
        #[doc(hidden)]
        pub static __LOGWISE_DOMAIN: $crate::LoggingDomain = $crate::LoggingDomain::new($enabled);
    };
}

/// Returns whether logging is enabled for a given [`Level`](crate::Level).
///
/// This macro centralizes the enablement logic used by the logging macros so the
/// same checks are available both inside and outside of the macros themselves.
#[macro_export]
macro_rules! log_enabled {
    ($level:expr) => {
        $crate::log_enabled!($level, || false)
    };
    ($level:expr, $domain:expr) => {{
        #[allow(unreachable_patterns)]
        match $level {
            $crate::Level::Trace => {
                #[cfg(debug_assertions)]
                {
                    $crate::context::Context::currently_tracing()
                }
                #[cfg(not(debug_assertions))]
                {
                    false
                }
            }
            $crate::Level::DebugInternal => {
                #[cfg(debug_assertions)]
                {
                    ($domain)() || $crate::context::Context::currently_tracing()
                }
                #[cfg(not(debug_assertions))]
                {
                    false
                }
            }
            $crate::Level::Info => {
                #[cfg(debug_assertions)]
                {
                    true
                }
                #[cfg(not(debug_assertions))]
                {
                    false
                }
            }
            $crate::Level::PerfWarn
            | $crate::Level::Warning
            | $crate::Level::Error
            | $crate::Level::Panic
            | $crate::Level::Analytics => true,
            _ => true,
        }
    }};
}

/// Formatter for writing log messages with full private information.
///
/// This formatter is used by the procedural macros to write both literal strings
/// and formatted values to a log record. It ensures that all values are logged
/// with their complete information (including private data) by calling
/// [`Loggable::log_all`](crate::privacy::Loggable::log_all).
///
/// # Example
///
/// This is typically used internally by the procedural macros:
///
/// ```rust
/// # use logwise::{LogRecord, Level};
/// # use logwise::hidden::PrivateFormatter;
/// # use logwise::privacy::Loggable;
/// let mut record = LogRecord::new(Level::Info);
/// let mut formatter = PrivateFormatter::new(&mut record);
/// formatter.write_literal("Count: ");
/// formatter.write_val(42u8);
/// ```
pub struct PrivateFormatter<'a> {
    record: &'a mut LogRecord,
}

impl<'a> PrivateFormatter<'a> {
    /// Creates a new `PrivateFormatter` for the given log record.
    ///
    /// # Arguments
    ///
    /// * `record` - The log record to write to
    #[inline]
    pub fn new(record: &'a mut LogRecord) -> Self {
        Self { record }
    }
    /// Writes a literal string to the log record.
    ///
    /// This is used for the static parts of log messages that don't contain
    /// any variable data.
    ///
    /// # Arguments
    ///
    /// * `s` - The literal string to write
    #[inline]
    pub fn write_literal(&mut self, s: &str) {
        self.record.log(s);
    }
    /// Writes a loggable value to the log record.
    ///
    /// This method calls [`Loggable::log_all`](crate::privacy::Loggable::log_all)
    /// to ensure the value is logged with all its information, including any
    /// private data. This is appropriate for local logging but not for remote
    /// telemetry.
    ///
    /// # Arguments
    ///
    /// * `s` - The value to log (must implement [`Loggable`](crate::privacy::Loggable))
    #[inline]
    pub fn write_val<Val: Loggable>(&mut self, s: Val) {
        s.log_all(self.record);
    }
}

/// Creates a log record for a `debuginternal` log message.
///
/// This function is called by the `debuginternal_sync!` and `debuginternal_async!`
/// macros to create the initial log record with metadata.
///
/// # Arguments
///
/// * `file` - The source file where the log was generated
/// * `line` - The line number in the source file
/// * `column` - The column number in the source file
///
/// # Returns
///
/// A [`LogRecord`] initialized with the current context and location information.
///
/// # Example
///
/// ```rust
/// # use logwise::hidden::{debuginternal_pre, debuginternal_sync_post};
/// // This is typically called by the macro, not directly
/// let record = debuginternal_pre(file!(), line!(), column!());
/// // ... formatter writes the message ...
/// debuginternal_sync_post(record);
/// ```
pub fn debuginternal_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    //safety: guarantee context won't change
    let mut record = crate::log_record::LogRecord::new(Level::DebugInternal);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);

    record.log("DEBUG: ");

    //file, line
    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    //for info, we can afford timestamp
    record.log_timestamp();

    record
}

/// Completes and dispatches a synchronous `debuginternal` log record.
///
/// This function sends the completed log record to all registered global loggers
/// synchronously. It's called by the `debuginternal_sync!` macro after the
/// message has been formatted.
///
/// # Arguments
///
/// * `record` - The completed log record to dispatch
///
/// # Example
///
/// ```rust
/// # use logwise::{LogRecord, Level};
/// # use logwise::hidden::debuginternal_sync_post;
/// let record = LogRecord::new(Level::DebugInternal);
/// // ... add message content ...
/// debuginternal_sync_post(record);
/// ```
pub fn debuginternal_sync_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record(record.clone());
    }
}

/// Completes and dispatches an asynchronous `debuginternal` log record.
///
/// This function sends the completed log record to all registered global loggers
/// asynchronously. It's called by the `debuginternal_async!` macro after the
/// message has been formatted.
///
/// # Arguments
///
/// * `record` - The completed log record to dispatch
///
/// # Example
///
/// ```rust
/// # use logwise::{LogRecord, Level};
/// # use logwise::hidden::debuginternal_async_post;
/// # async fn example() {
/// let record = LogRecord::new(Level::DebugInternal);
/// // ... add message content ...
/// debuginternal_async_post(record).await;
/// # }
/// ```
pub async fn debuginternal_async_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record_async(record.clone()).await;
    }
}

/// Creates a log record for an `info` log message.
///
/// This function is called by the `info_sync!` and `info_async!` macros to create
/// the initial log record with metadata. Info logs are intended for supporting
/// downstream crates and are only available in debug builds.
///
/// # Arguments
///
/// * `file` - The source file where the log was generated
/// * `line` - The line number in the source file
/// * `column` - The column number in the source file
///
/// # Returns
///
/// A [`LogRecord`] initialized with the current context, location, and timestamp.
///
/// # Example
///
/// ```rust
/// # use logwise::hidden::{info_sync_pre, info_sync_post};
/// // This is typically called by the macro, not directly
/// let record = info_sync_pre(file!(), line!(), column!());
/// // ... formatter writes the message ...
/// info_sync_post(record);
/// ```
pub fn info_sync_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    //safety: guarantee context won't change
    let mut record = crate::log_record::LogRecord::new(Level::Info);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);

    record.log("INFO: ");

    //file, line
    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    //for info, we can afford timestamp
    record.log_timestamp();
    record
}

/// Completes and dispatches a synchronous `info` log record.
///
/// This function sends the completed log record to all registered global loggers
/// synchronously. It's called by the `info_sync!` macro after the message has
/// been formatted.
///
/// # Arguments
///
/// * `record` - The completed log record to dispatch
///
/// # Example
///
/// ```rust
/// # use logwise::{LogRecord, Level};
/// # use logwise::hidden::info_sync_post;
/// let record = LogRecord::new(Level::Info);
/// // ... add message content ...
/// info_sync_post(record);
/// ```
pub fn info_sync_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        //can't call eprintln in wasm32!
        // eprintln!("Sending to logger: {:?}", logger);
        logger.finish_log_record(record.clone());
    }
}

/// Completes and dispatches an asynchronous `info` log record.
///
/// This function sends the completed log record to all registered global loggers
/// asynchronously. It's called by the `info_async!` macro after the message has
/// been formatted.
///
/// # Arguments
///
/// * `record` - The completed log record to dispatch
///
/// # Example
///
/// ```rust
/// # use logwise::{LogRecord, Level};
/// # use logwise::hidden::info_async_post;
/// # async fn example() {
/// let record = LogRecord::new(Level::Info);
/// // ... add message content ...
/// info_async_post(record).await;
/// # }
/// ```
pub async fn info_async_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record_async(record.clone()).await;
    }
}

/// Creates a log record for a `warning` log message.
///
/// This function is called by the `warn_sync!` macro to create the initial log
/// record with metadata. Warning logs are for suspicious conditions and are
/// available in all build types.
///
/// # Arguments
///
/// * `file` - The source file where the log was generated
/// * `line` - The line number in the source file
/// * `column` - The column number in the source file
///
/// # Returns
///
/// A [`LogRecord`] initialized with the current context, location, and timestamp.
///
/// # Example
///
/// ```rust
/// # use logwise::hidden::{warn_sync_pre, warn_sync_post};
/// // This is typically called by the macro, not directly
/// let record = warn_sync_pre(file!(), line!(), column!());
/// // ... formatter writes the message ...
/// warn_sync_post(record);
/// ```
pub fn warn_sync_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    //safety: guarantee context won't change
    let mut record = crate::log_record::LogRecord::new(Level::Warning);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);

    record.log("WARN: ");

    //file, line
    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    //for warn, we can afford timestamp
    record.log_timestamp();
    record
}

/// Completes and dispatches a synchronous `warning` log record.
///
/// This function sends the completed log record to all registered global loggers
/// synchronously. It's called by the `warn_sync!` macro after the message has
/// been formatted.
///
/// # Arguments
///
/// * `record` - The completed log record to dispatch
///
/// # Example
///
/// ```rust
/// # use logwise::{LogRecord, Level};
/// # use logwise::hidden::warn_sync_post;
/// let record = LogRecord::new(Level::Warning);
/// // ... add message content ...
/// warn_sync_post(record);
/// ```
pub fn warn_sync_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record(record.clone());
    }
}

/// Creates a log record for a `trace` log message.
///
/// This function is called by the `trace_sync!` and `trace_async!` macros to create
/// the initial log record with metadata. Trace logs are for detailed debugging and
/// are only available in debug builds with per-thread activation.
///
/// # Arguments
///
/// * `file` - The source file where the log was generated
/// * `line` - The line number in the source file
/// * `column` - The column number in the source file
///
/// # Returns
///
/// A [`LogRecord`] initialized with the current context, location, and timestamp.
///
/// # Example
///
/// ```rust
/// # use logwise::hidden::{trace_sync_pre, trace_sync_post};
/// // This is typically called by the macro, not directly
/// let record = trace_sync_pre(file!(), line!(), column!());
/// // ... formatter writes the message ...
/// trace_sync_post(record);
/// ```
pub fn trace_sync_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    //safety: guarantee context won't change
    let mut record = crate::log_record::LogRecord::new(Level::Trace);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);

    record.log("TRACE: ");

    //file, line
    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    //for warn, we can afford timestamp
    record.log_timestamp();
    record
}

/// Completes and dispatches a synchronous `trace` log record.
///
/// This function sends the completed log record to all registered global loggers
/// synchronously. It's called by the `trace_sync!` macro after the message has
/// been formatted.
///
/// # Arguments
///
/// * `record` - The completed log record to dispatch
///
/// # Example
///
/// ```rust
/// # use logwise::{LogRecord, Level};
/// # use logwise::hidden::trace_sync_post;
/// let record = LogRecord::new(Level::Trace);
/// // ... add message content ...
/// trace_sync_post(record);
/// ```
pub fn trace_sync_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record(record.clone());
    }
}

/// Completes and dispatches an asynchronous `trace` log record.
///
/// This function sends the completed log record to all registered global loggers
/// asynchronously. It's called by the `trace_async!` macro after the message has
/// been formatted.
///
/// # Arguments
///
/// * `record` - The completed log record to dispatch
///
/// # Example
///
/// ```rust
/// # use logwise::{LogRecord, Level};
/// # use logwise::hidden::trace_async_post;
/// # async fn example() {
/// let record = LogRecord::new(Level::Trace);
/// // ... add message content ...
/// trace_async_post(record).await;
/// # }
/// ```
pub async fn trace_async_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record_async(record.clone()).await;
    }
}

/// Creates a log record for an `error` log message.
///
/// This function is called by the `error_sync!` and `error_async!` macros to create
/// the initial log record with metadata. Error logs are for logging errors in Results
/// and are available in all build types.
///
/// # Arguments
///
/// * `file` - The source file where the log was generated
/// * `line` - The line number in the source file
/// * `column` - The column number in the source file
///
/// # Returns
///
/// A [`LogRecord`] initialized with the current context, location, and timestamp.
///
/// # Example
///
/// ```rust
/// # use logwise::hidden::{error_sync_pre, error_sync_post};
/// // This is typically called by the macro, not directly
/// let record = error_sync_pre(file!(), line!(), column!());
/// // ... formatter writes the message ...
/// error_sync_post(record);
/// ```
pub fn error_sync_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    //safety: guarantee context won't change
    let mut record = crate::log_record::LogRecord::new(Level::Error);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);

    record.log("ERROR: ");

    //file, line
    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    //for warn, we can afford timestamp
    record.log_timestamp();
    record
}

/// Completes and dispatches a synchronous `error` log record.
///
/// This function sends the completed log record to all registered global loggers
/// synchronously. It's called by the `error_sync!` macro after the message has
/// been formatted.
///
/// # Arguments
///
/// * `record` - The completed log record to dispatch
///
/// # Example
///
/// ```rust
/// # use logwise::{LogRecord, Level};
/// # use logwise::hidden::error_sync_post;
/// let record = LogRecord::new(Level::Error);
/// // ... add message content ...
/// error_sync_post(record);
/// ```
pub fn error_sync_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record(record.clone());
    }
}

/// Completes and dispatches an asynchronous `error` log record.
///
/// This function sends the completed log record to all registered global loggers
/// asynchronously. It's called by the `error_async!` macro after the message has
/// been formatted.
///
/// # Arguments
///
/// * `record` - The completed log record to dispatch
///
/// # Example
///
/// ```rust
/// # use logwise::{LogRecord, Level};
/// # use logwise::hidden::error_async_post;
/// # async fn example() {
/// let record = LogRecord::new(Level::Error);
/// // ... add message content ...
/// error_async_post(record).await;
/// # }
/// ```
pub async fn error_async_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record_async(record.clone()).await;
    }
}

/// Creates a log record for the beginning of a performance warning interval.
///
/// This function is called by the `perfwarn!` and `perfwarn_begin!` macros to
/// create the initial log record that marks the start of a performance measurement.
/// The corresponding end of the interval is logged when the [`PerfwarnInterval`](crate::interval::PerfwarnInterval)
/// is dropped.
///
/// # Arguments
///
/// * `file` - The source file where the log was generated
/// * `line` - The line number in the source file
/// * `column` - The column number in the source file
///
/// # Returns
///
/// A [`LogRecord`] initialized with the current context and the start time.
///
/// # Example
///
/// ```rust
/// # use logwise::hidden::{perfwarn_begin_pre, perfwarn_begin_post};
/// // This is typically called by the macro, not directly
/// let record = perfwarn_begin_pre(file!(), line!(), column!());
/// // ... formatter writes the message ...
/// let interval = perfwarn_begin_post(record, "operation_name");
/// // ... code to measure ...
/// drop(interval); // Logs the end of the interval
/// ```
pub fn perfwarn_begin_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    let start = crate::sys::Instant::now();

    //safety: guarantee context won't change
    let mut record = crate::log_record::LogRecord::new(Level::PerfWarn);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);

    record.log("PERFWARN: BEGIN ");

    //file, line
    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    record.log_time_since(start);
    record
}

/// Completes the beginning log record and creates a performance warning interval.
///
/// This function sends the initial log record to all registered global loggers and
/// returns a [`PerfwarnInterval`](crate::interval::PerfwarnInterval) that will log
/// the completion when dropped.
///
/// # Arguments
///
/// * `record` - The completed log record for the interval start
/// * `name` - The name of the operation being measured
///
/// # Returns
///
/// A [`PerfwarnInterval`](crate::interval::PerfwarnInterval) that tracks the duration
/// and logs completion when dropped.
///
/// # Example
///
/// ```rust
/// # use logwise::{LogRecord, Level};
/// # use logwise::hidden::perfwarn_begin_post;
/// let record = LogRecord::new(Level::PerfWarn);
/// // ... add message content ...
/// let interval = perfwarn_begin_post(record, "database_query");
/// // ... perform operation ...
/// drop(interval); // Automatically logs the end
/// ```
pub fn perfwarn_begin_post(
    record: LogRecord,
    name: &'static str,
) -> crate::interval::PerfwarnInterval {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record(record.clone());
    }

    crate::interval::PerfwarnInterval::new(name, crate::sys::Instant::now())
}

/// Completes the beginning log record and creates a conditional performance warning interval.
///
/// This function returns a [`PerfwarnIntervalIf`](crate::interval::PerfwarnIntervalIf) that will log
/// the completion when dropped, but only if the duration exceeds the threshold.
///
/// # Arguments
///
/// * `record` - The prepared log record for the interval
/// * `name` - The name of the operation being measured
/// * `threshold` - The duration threshold for logging
///
/// # Returns
///
/// A [`PerfwarnIntervalIf`](crate::interval::PerfwarnIntervalIf) that tracks the duration
/// and conditionally logs completion when dropped.
pub fn perfwarn_begin_if_post(
    record: LogRecord,
    name: &'static str,
    threshold: crate::sys::Duration,
) -> crate::interval::PerfwarnIntervalIf {
    // We do NOT log the record here. It is deferred until drop.
    crate::interval::PerfwarnIntervalIf::new(name, crate::sys::Instant::now(), threshold, record)
}

/// Creates a log record for a conditional performance warning interval.
///
/// This function is called by the `perfwarn_begin_if!` macro to create the initial
/// log record. Unlike `perfwarn_begin_pre`, this does not log "BEGIN" or a timestamp
/// immediately, as logging is deferred until the interval completes.
///
/// # Arguments
///
/// * `file` - The source file where the log was generated
/// * `line` - The line number in the source file
/// * `column` - The column number in the source file
///
/// # Returns
///
/// A [`LogRecord`] initialized with the current context and location information.
pub fn perfwarn_begin_if_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    //safety: guarantee context won't change
    let mut record = crate::log_record::LogRecord::new(Level::PerfWarn);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);

    record.log("PERFWARN: ");

    //file, line
    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    record
}

#[cfg(test)]
mod tests {
    use logwise::context::Context;
    use logwise_proc::{debuginternal_sync, info_sync, warn_sync};

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_warn_sync() {
        crate::context::Context::reset("test_warn_sync".to_string());
        info_sync!("test_warn_sync");
        warn_sync!("test_warn_sync Hello {world}!", world = 23);
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn perfwarn() {
        use logwise::perfwarn;
        Context::reset("test_perfwarn".to_string());
        info_sync!("test_perfwarn");
        let _: i32 = perfwarn!("test_perfwarn interval name", {
            //code to profile
            23
        });
    }
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_debuginternal_sync() {
        crate::context::Context::reset("test_debuginternal_sync".to_string());
        debuginternal_sync!("test_debuginternal_sync");
    }
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_log_rich() {
        let val = false;
        crate::context::Context::reset("test_log_rich".to_string());

        debuginternal_sync!("Hello {world}!", world = val);
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_log_custom() {
        crate::context::Context::reset("test_log_custom".to_string());
        #[derive(Debug)]
        #[allow(dead_code)]
        struct S(i32);
        let s = S(23);
        debuginternal_sync!("{s}!", s = logwise::privacy::LogIt(&s));
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_log_info_async() {
        crate::context::Context::reset("test_log_info_async".to_string());
        let _ = async {
            logwise::info_async!("test_log_info_async");
        };
        //I guess we do something with this, but we can't call truntime because it depends on us...
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_trace() {
        crate::context::Context::reset("test_trace".to_string());
        logwise::trace_sync!("test_trace");
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_logging_domain_trait_implementations() {
        use crate::LoggingDomain;

        // Test Debug implementation
        let domain_enabled = LoggingDomain::new(true);
        let domain_disabled = LoggingDomain::new(false);
        let debug_enabled = format!("{:?}", domain_enabled);
        let debug_disabled = format!("{:?}", domain_disabled);
        assert!(debug_enabled.contains("LoggingDomain"));
        assert!(debug_disabled.contains("LoggingDomain"));

        // Test Display implementation
        let display_enabled = format!("{}", domain_enabled);
        let display_disabled = format!("{}", domain_disabled);
        assert_eq!(display_enabled, "LoggingDomain(internal=true)");
        assert_eq!(display_disabled, "LoggingDomain(internal=false)");

        // Test Default implementation
        let default_domain = LoggingDomain::default();
        assert!(!default_domain.is_internal());

        // Test From<bool> implementation
        let domain_from_true: LoggingDomain = true.into();
        let domain_from_false: LoggingDomain = false.into();
        assert!(domain_from_true.is_internal());
        assert!(!domain_from_false.is_internal());
    }
}
