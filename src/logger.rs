// SPDX-License-Identifier: MIT OR Apache-2.0

//! Core logger trait and infrastructure for the logwise logging system.
//!
//! This module defines the fundamental [`Logger`] trait that all logging backends must implement.
//! The trait provides a simple yet flexible interface for processing log records, supporting both
//! synchronous and asynchronous logging patterns.
//!
//! # Architecture
//!
//! The `Logger` trait is the core abstraction that enables logwise to support multiple logging
//! backends simultaneously. Each logger implementation receives [`LogRecord`] instances containing
//! the formatted log data and is responsible for outputting them to its destination (stderr, files,
//! network, memory, etc.).
//!
//! ## Design Principles
//!
//! - **Simplicity**: The trait has only three required methods, making it easy to implement custom loggers
//! - **Flexibility**: Supports both sync and async logging patterns for different runtime environments
//! - **Thread Safety**: All loggers must be `Send + Sync` to work in multi-threaded environments
//! - **Zero Allocation**: The [`LogRecord`] is passed by value to avoid unnecessary allocations
//! - **Graceful Shutdown**: The `prepare_to_die` method ensures buffers are flushed before exit
//!
//! # Built-in Implementations
//!
//! logwise provides several logger implementations:
//!
//! - [`StdErrorLogger`](crate::StdErrorLogger): Outputs to stderr (default logger)
//! - [`InMemoryLogger`](crate::InMemoryLogger): Stores logs in memory for testing
//! - [`LocalLogger`](crate::local_logger::LocalLogger): Thread-local logging for performance
//!
//! # Custom Logger Implementation
//!
//! To create a custom logger, implement the `Logger` trait:
//!
//! ```
//! use logwise::{Logger, LogRecord};
//! use std::sync::Mutex;
//!
//! #[derive(Debug)]
//! struct FileLogger {
//!     file: Mutex<std::fs::File>,
//! }
//!
//! impl FileLogger {
//!     fn new(path: &str) -> std::io::Result<Self> {
//!         use std::fs::OpenOptions;
//!         let file = OpenOptions::new()
//!             .create(true)
//!             .append(true)
//!             .open(path)?;
//!         Ok(Self {
//!             file: Mutex::new(file),
//!         })
//!     }
//! }
//!
//! impl Logger for FileLogger {
//!     fn finish_log_record(&self, record: LogRecord) {
//!         use std::io::Write;
//!         let mut file = self.file.lock().unwrap();
//!         writeln!(file, "{}", record).ok();
//!     }
//!
//!     fn finish_log_record_async<'s>(
//!         &'s self,
//!         record: LogRecord,
//!     ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 's>> {
//!         // Simple async wrapper around sync implementation
//!         Box::pin(async move {
//!             self.finish_log_record(record);
//!         })
//!     }
//!
//!     fn prepare_to_die(&self) {
//!         use std::io::Write;
//!         let mut file = self.file.lock().unwrap();
//!         file.flush().ok();
//!     }
//! }
//! ```
//!
//! # Global Logger Management
//!
//! Loggers are typically registered globally using the [`global_logger`](crate::global_logger) module:
//!
//! ```
//! logwise::declare_logging_domain!();
//! # fn main() {
//! use logwise::{Logger, add_global_logger};
//! use std::sync::Arc;
//!
//! # #[derive(Debug)]
//! # struct MyCustomLogger;
//! # impl Logger for MyCustomLogger {
//! #     fn finish_log_record(&self, _: logwise::LogRecord) {}
//! #     fn finish_log_record_async<'s>(&'s self, record: logwise::LogRecord)
//! #         -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 's>> {
//! #         Box::pin(async move { self.finish_log_record(record) })
//! #     }
//! #     fn prepare_to_die(&self) {}
//! # }
//!
//! // Create and register a custom logger
//! let logger = Arc::new(MyCustomLogger);
//! add_global_logger(logger);
//!
//! // Now all log messages will also go to MyCustomLogger
//! logwise::info_sync!("This message goes to all registered loggers");
//! # }
//! ```
//!
//! # Performance Considerations
//!
//! - The `finish_log_record` method should be fast as it may be called frequently
//! - Consider buffering writes in your logger implementation for better performance
//! - Use the async variant when in async contexts to avoid blocking the executor
//! - The `prepare_to_die` method is called rarely, so it can do more expensive cleanup
//!
//! # Thread Safety
//!
//! All `Logger` implementations must be thread-safe (`Send + Sync`). This typically means:
//! - Using `Mutex` or `RwLock` for mutable state
//! - Using atomic types for simple counters
//! - Ensuring any file handles or network connections are properly synchronized

use crate::log_record::LogRecord;
use std::fmt::Debug;

/// Core trait for implementing logging backends in logwise.
///
/// This trait defines the interface that all loggers must implement to receive and process
/// log records from the logwise logging system. Implementations can output logs to any
/// destination: stderr, files, network services, or in-memory buffers.
///
/// # Requirements
///
/// All implementations must be:
/// - **Thread-safe**: The logger will be called from multiple threads simultaneously
/// - **Send + Sync**: Required for use in multi-threaded environments
/// - **Debug**: For diagnostic purposes and error reporting
///
/// # Example Implementation
///
/// ```
/// use logwise::{Logger, LogRecord};
/// use std::sync::atomic::{AtomicUsize, Ordering};
///
/// #[derive(Debug)]
/// struct CountingLogger {
///     count: AtomicUsize,
/// }
///
/// impl CountingLogger {
///     fn new() -> Self {
///         Self {
///             count: AtomicUsize::new(0),
///         }
///     }
///
///     fn get_count(&self) -> usize {
///         self.count.load(Ordering::Relaxed)
///     }
/// }
///
/// impl Logger for CountingLogger {
///     fn finish_log_record(&self, _record: LogRecord) {
///         self.count.fetch_add(1, Ordering::Relaxed);
///         // In a real implementation, you would process the record here
///     }
///
///     fn finish_log_record_async<'s>(
///         &'s self,
///         record: LogRecord,
///     ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 's>> {
///         Box::pin(async move {
///             self.finish_log_record(record);
///         })
///     }
///
///     fn prepare_to_die(&self) {
///         // No cleanup needed for this simple logger
///     }
/// }
///
/// // Usage
/// let logger = CountingLogger::new();
/// let mut record = LogRecord::new(logwise::Level::Info);
/// record.log("Test message");
/// logger.finish_log_record(record);
/// assert_eq!(logger.get_count(), 1);
/// ```
pub trait Logger: Debug + Send + Sync {
    /// Processes a log record synchronously.
    ///
    /// This method receives a complete [`LogRecord`] and is responsible for outputting it
    /// to the logger's destination. The implementation should be as fast as possible since
    /// it may be called frequently and from performance-critical code paths.
    ///
    /// # Parameters
    ///
    /// * `record` - The log record containing the formatted message and metadata
    ///
    /// # Implementation Notes
    ///
    /// - This method may be called from multiple threads simultaneously
    /// - The implementation should not panic; errors should be handled gracefully
    /// - Consider buffering writes for better performance
    /// - The record is passed by value to avoid lifetime issues
    ///
    /// # Examples
    ///
    /// ```
    /// use logwise::{Logger, LogRecord, Level};
    ///
    /// # #[derive(Debug)]
    /// # struct ConsoleLogger;
    /// impl Logger for ConsoleLogger {
    ///     fn finish_log_record(&self, record: LogRecord) {
    ///         // Simple console output
    ///         println!("{}", record);
    ///     }
    ///     # fn finish_log_record_async<'s>(&'s self, record: LogRecord)
    ///     #     -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 's>> {
    ///     #     Box::pin(async move { self.finish_log_record(record) })
    ///     # }
    ///     # fn prepare_to_die(&self) {}
    /// }
    /// ```
    fn finish_log_record(&self, record: LogRecord);

    /// Processes a log record asynchronously.
    ///
    /// This method provides an async interface for processing log records, allowing loggers
    /// to leverage existing async contexts and avoid blocking async executors. This is
    /// particularly useful for loggers that perform I/O operations or need to integrate
    /// with async-first systems.
    ///
    /// # Parameters
    ///
    /// * `record` - The log record containing the formatted message and metadata
    ///
    /// # Returns
    ///
    /// A pinned future that completes when the log record has been processed.
    /// The future must be `Send` to work across thread boundaries.
    ///
    /// # Implementation Options
    ///
    /// Loggers have two common implementation strategies:
    ///
    /// 1. **Simple wrapper**: For loggers that don't benefit from async, wrap the sync method:
    ///    ```
    ///    # use logwise::{Logger, LogRecord};
    ///    # #[derive(Debug)]
    ///    # struct MyLogger;
    ///    # impl Logger for MyLogger {
    ///    #     fn finish_log_record(&self, _record: LogRecord) {}
    ///    fn finish_log_record_async<'s>(
    ///        &'s self,
    ///        record: LogRecord,
    ///    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 's>> {
    ///        Box::pin(async move {
    ///            self.finish_log_record(record)
    ///        })
    ///    }
    ///    #     fn prepare_to_die(&self) {}
    ///    # }
    ///    ```
    ///
    /// 2. **True async**: For loggers that benefit from async I/O:
    ///    ```
    ///    # use logwise::{Logger, LogRecord};
    ///    # #[derive(Debug)]
    ///    # struct AsyncLogger;
    ///    # impl Logger for AsyncLogger {
    ///    #     fn finish_log_record(&self, _record: LogRecord) {}
    ///    fn finish_log_record_async<'s>(
    ///        &'s self,
    ///        record: LogRecord,
    ///    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 's>> {
    ///        Box::pin(async move {
    ///            // Perform actual async I/O here
    ///            // e.g., async_write_to_file(record).await
    ///            todo!("Implement async logging")
    ///        })
    ///    }
    ///    #     fn prepare_to_die(&self) {}
    ///    # }
    ///    ```
    ///
    /// # Lifetime Notes
    ///
    /// The lifetime parameter `'s` ties the returned future to the logger's lifetime,
    /// ensuring the logger remains valid while the future is being polled.
    fn finish_log_record_async<'s>(
        &'s self,
        record: LogRecord,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 's>>;

    /// Prepares the logger for application shutdown.
    ///
    /// This method is called when the application is about to exit, giving the logger
    /// a chance to flush any buffered data, close file handles, send final network
    /// packets, or perform other cleanup operations. This ensures no log data is lost
    /// during shutdown.
    ///
    /// # When This Is Called
    ///
    /// - Before normal application exit
    /// - During panic handling (if possible)
    /// - When explicitly requested by the application
    ///
    /// # Implementation Guidelines
    ///
    /// - Flush all buffered data to persistent storage
    /// - Close network connections gracefully
    /// - Release any system resources
    /// - This method may take longer than `finish_log_record` since it's called rarely
    /// - Should not panic; handle errors gracefully
    ///
    /// # Examples
    ///
    /// ```
    /// use logwise::{Logger, LogRecord};
    /// use std::sync::Mutex;
    ///
    /// #[derive(Debug)]
    /// struct BufferedLogger {
    ///     buffer: Mutex<Vec<String>>,
    /// }
    ///
    /// impl BufferedLogger {
    ///     fn new() -> Self {
    ///         Self {
    ///             buffer: Mutex::new(Vec::new()),
    ///         }
    ///     }
    /// }
    ///
    /// impl Logger for BufferedLogger {
    ///     fn finish_log_record(&self, record: LogRecord) {
    ///         let mut buffer = self.buffer.lock().unwrap();
    ///         buffer.push(record.to_string());
    ///     }
    ///
    ///     fn finish_log_record_async<'s>(
    ///         &'s self,
    ///         record: LogRecord,
    ///     ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 's>> {
    ///         Box::pin(async move {
    ///             self.finish_log_record(record);
    ///         })
    ///     }
    ///
    ///     fn prepare_to_die(&self) {
    ///         // Flush buffer to stdout before exit
    ///         let buffer = self.buffer.lock().unwrap();
    ///         for message in buffer.iter() {
    ///             println!("{}", message);
    ///         }
    ///     }
    /// }
    /// ```
    fn prepare_to_die(&self);
}

// Implementation notes:
//
// The Logger trait intentionally does not implement Clone, as loggers often hold unique
// resources (file handles, network connections) that shouldn't be duplicated.
//
// PartialEq/Eq are not implemented because equality semantics for loggers are unclear
// (do we compare configuration, destination, or identity?).
//
// Default is not implemented as loggers typically require configuration (output paths,
// network endpoints, etc.) that cannot be reasonably defaulted.
//
// Send + Sync are required to enable multi-threaded logging, which is essential for
// most Rust applications. Loggers that cannot be made thread-safe should not be used
// with logwise.
