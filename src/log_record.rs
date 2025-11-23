// SPDX-License-Identifier: MIT OR Apache-2.0

//! Log record type for the logwise logging system.
//!
//! This module defines [`LogRecord`], the core data structure that accumulates log message
//! parts during the logging process. Log records are built incrementally using the `log`
//! and `log_owned` methods, then submitted to loggers for output.
//!
//! # Design Philosophy
//!
//! The `LogRecord` type is designed to minimize allocations during logging. Instead of
//! concatenating strings, it stores parts separately and only joins them when needed for
//! final output. This approach:
//!
//! - Reduces memory allocation overhead
//! - Enables efficient pass-by-value to loggers
//! - Supports thread-safe logging without shared buffers
//!
//! # Usage Pattern
//!
//! 1. Create a new `LogRecord` with a log level
//! 2. Progressively add message parts using `log()` or `log_owned()`
//! 3. Submit the complete record to loggers via `Logger::finish_log_record()`
//!
//! # Example
//!
//! ```rust
//! use logwise::{LogRecord, Level};
//!
//! // Create a new record
//! let mut record = LogRecord::new(Level::Info);
//!
//! // Add message parts
//! record.log("Processing request ");
//! record.log_owned(format!("#{}", 42));
//! record.log(" completed");
//!
//! // The record can now be sent to loggers
//! // println!("{}", record); // Output: "Processing request #42 completed"
//! ```

use crate::Level;
use std::fmt::{Debug, Display};
use std::sync::OnceLock;

static INITIAL_TIMESTAMP: OnceLock<crate::sys::Instant> = OnceLock::new();

fn initial_timestamp() -> crate::sys::Instant {
    *INITIAL_TIMESTAMP.get_or_init(crate::sys::Instant::now)
}

/**
A log record.

We'd like to construct our API in a way that we don't need to allocate memory by concatenating strings, etc.

So instead our API assumes you progressively write a lot into somewhere.  However, due to the multithreaded
nature of logging, we need to be able to write to a buffer that is not shared between threads.

Instead, the design is as follows:

1.  Create a new [LogRecord].
2.  Progressively write to the [LogRecord].
3.  Finish the [LogRecord] and submit it to the [logwise::logger::Logger].

*/
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LogRecord {
    pub(crate) parts: Vec<String>,
    level: Level,
}
impl LogRecord {
    /**
    Append the message to the record.

    This is called in the case that a message is not already owned.
    */
    pub fn log(&mut self, message: &str) {
        self.parts.push(message.to_string());
    }

    /**
    Append the message to the record, taking ownership of the message.

    This is useful for messages that are already owned, such as those that are constructed in the process of logging.
    Logging implementations may choose to copy and drop the value if desired.
    */
    pub fn log_owned(&mut self, message: String) {
        self.parts.push(message);
    }

    pub fn new(level: Level) -> Self {
        Self {
            parts: Vec::new(),
            level,
        }
    }

    /**
    Log the current time to the record, followed by a space.
    */
    pub fn log_timestamp(&mut self) -> crate::sys::Instant {
        let time = crate::sys::Instant::now();
        let duration = time.duration_since(initial_timestamp());
        self.log_owned(format!("[{:?}] ", duration));
        time
    }

    pub fn log_time_since(&mut self, start: crate::sys::Instant) {
        let duration = start.duration_since(initial_timestamp());
        self.log_owned(format!("[{:?}] ", duration));
    }
    pub fn level(&self) -> Level {
        self.level
    }
}

impl Default for LogRecord {
    fn default() -> Self {
        Self::new(Level::Info)
    }
}

impl Display for LogRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for part in &self.parts {
            write!(f, "{}", part)?;
        }
        Ok(())
    }
}
/*
Boilerplate notes for LogRecord:

IMPLEMENTED:
- Debug: Derived - essential for diagnostics
- Clone: Derived - useful for record duplication/forwarding
- PartialEq/Eq: Derived - enables record comparison and deduplication
- Hash: Derived - consistent with Eq, enables use in hash collections
- Default: Implemented - provides sensible zero-value (Info level, empty parts)
- Display: Implemented - formats record parts for output

NOT IMPLEMENTED:
- Copy: Vec<String> contains heap-allocated data, not suitable for Copy
- Ord/PartialOrd: No meaningful ordering for log records
- From/Into: No obvious conversions to/from other types
- AsRef/AsMut: No clear underlying type to reference
- Deref: Must deref to a pointer type, which LogRecord doesn't naturally provide

AUTOMATIC:
- Send: Automatically implemented - Vec<String> and Level are Send
- Sync: NOT automatically implemented - Vec<String> is not Sync (but records
  are typically owned by single threads during construction anyway)
*/
