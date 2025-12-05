// SPDX-License-Identifier: MIT OR Apache-2.0

//! Log record dispatch functions for the logwise logging library.
//!
//! This module provides the implementation functions that are called by the
//! procedural macros in `logwise_proc`. These functions handle the creation
//! and dispatching of log records to global loggers.
//!
//! # Architecture
//!
//! The logging flow follows this pattern:
//! 1. A `*_pre` function creates a [`LogRecord`] with appropriate metadata
//! 2. The procedural macro uses [`PrivateFormatter`](crate::hidden::PrivateFormatter) to add the formatted message
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

use crate::Level;
use crate::log_record::LogRecord;

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

/// Creates a log record for a `mandatory` log message.
///
/// This function is called by the `mandatory_sync!` and `mandatory_async!` macros to create
/// the initial log record with metadata. Mandatory logs are always enabled, even in release
/// builds, intended for temporary printf-style debugging in hostile environments.
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
pub fn mandatory_sync_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    let mut record = crate::log_record::LogRecord::new(Level::Mandatory);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);

    record.log("MANDATORY: ");

    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    record.log_timestamp();
    record
}

/// Completes and dispatches a synchronous `mandatory` log record.
///
/// This function sends the completed log record to all registered global loggers
/// synchronously. It's called by the `mandatory_sync!` macro after the message has
/// been formatted.
///
/// # Arguments
///
/// * `record` - The completed log record to dispatch
pub fn mandatory_sync_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record(record.clone());
    }
}

/// Completes and dispatches an asynchronous `mandatory` log record.
///
/// This function sends the completed log record to all registered global loggers
/// asynchronously. It's called by the `mandatory_async!` macro after the message has
/// been formatted.
///
/// # Arguments
///
/// * `record` - The completed log record to dispatch
pub async fn mandatory_async_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record_async(record.clone()).await;
    }
}

/// Creates a log record for a `profile` log message.
///
/// This function is called by the `profile_sync!` and `profile_async!` macros to create
/// the initial log record with metadata. Profile logs are always enabled, even in release
/// builds, intended for temporary profiling and performance investigation.
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
pub fn profile_sync_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    let mut record = crate::log_record::LogRecord::new(Level::Profile);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);

    record.log("PROFILE: ");

    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    record.log_timestamp();
    record
}

/// Completes and dispatches a synchronous `profile` log record.
///
/// This function sends the completed log record to all registered global loggers
/// synchronously. It's called by the `profile_sync!` macro after the message has
/// been formatted.
///
/// # Arguments
///
/// * `record` - The completed log record to dispatch
pub fn profile_sync_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record(record.clone());
    }
}

/// Completes and dispatches an asynchronous `profile` log record.
///
/// This function sends the completed log record to all registered global loggers
/// asynchronously. It's called by the `profile_async!` macro after the message has
/// been formatted.
///
/// # Arguments
///
/// * `record` - The completed log record to dispatch
pub async fn profile_async_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record_async(record.clone()).await;
    }
}

/// Creates a log record for the beginning of a profile interval.
///
/// This function is called by the `profile_begin!` macro to create the initial
/// log record that marks the start of a profiling measurement. It generates a
/// unique ID for correlating BEGIN and END messages.
///
/// # Arguments
///
/// * `file` - The source file where the log was generated
/// * `line` - The line number in the source file
/// * `column` - The column number in the source file
///
/// # Returns
///
/// A tuple of (unique ID, [`LogRecord`]) initialized with the current context
/// and the start time.
pub fn profile_begin_pre(file: &'static str, line: u32, column: u32) -> (u64, LogRecord) {
    let id = crate::interval::next_profile_id();
    let start = crate::sys::Instant::now();

    let mut record = crate::log_record::LogRecord::new(Level::Profile);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);

    record.log_owned(format!("PROFILE: BEGIN [id={}] ", id));

    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    record.log_time_since(start);

    (id, record)
}

/// Completes the beginning log record and creates a profile interval.
///
/// This function sends the initial log record to all registered global loggers and
/// returns a [`ProfileInterval`](crate::interval::ProfileInterval) that will log
/// the completion when dropped.
///
/// # Arguments
///
/// * `id` - The unique ID for this profile interval
/// * `record` - The completed log record for the interval start
/// * `name` - The name of the operation being profiled
///
/// # Returns
///
/// A [`ProfileInterval`](crate::interval::ProfileInterval) that logs the end
/// message with elapsed duration when dropped.
pub fn profile_begin_post(
    id: u64,
    record: LogRecord,
    name: &'static str,
) -> crate::interval::ProfileInterval {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record(record.clone());
    }

    crate::interval::ProfileInterval::new(id, name, crate::sys::Instant::now())
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
    #[allow(clippy::let_underscore_future)] // Just testing macro compiles, not executing
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
    fn test_mandatory_sync() {
        crate::context::Context::reset("test_mandatory_sync".to_string());
        logwise::mandatory_sync!("mandatory debug value={val}", val = 42);
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profile_sync() {
        crate::context::Context::reset("test_profile_sync".to_string());
        logwise::profile_sync!("profile timing={ms}ms", ms = 100);
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profile_begin() {
        crate::context::Context::reset("test_profile_begin".to_string());
        let interval = logwise::profile_begin!("test_operation {param}", param = "foo");
        // Simulate some work
        let _ = (0..100).sum::<i32>();
        drop(interval);
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profile_begin_nested() {
        crate::context::Context::reset("test_profile_begin_nested".to_string());
        let outer = logwise::profile_begin!("outer_operation");
        {
            let inner = logwise::profile_begin!("inner_operation");
            let _ = (0..50).sum::<i32>();
            drop(inner);
        }
        drop(outer);
    }
}
