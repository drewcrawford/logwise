// SPDX-License-Identifier: MIT OR Apache-2.0

//! # In-Memory Logger
//!
//! This module provides an in-memory logging implementation for testing and debugging purposes.
//! The `InMemoryLogger` captures log records in memory rather than writing them to stderr or
//! other outputs, making it ideal for:
//!
//! - Unit testing code that uses logwise logging
//! - Capturing logs in environments where stderr is redirected or unavailable
//! - Programmatically examining log output
//! - Debugging in adversarial environments (e.g., WASM in browsers)
//!
//! ## Architecture
//!
//! The logger uses an `Arc<Mutex<Vec<String>>>` internally to store log messages safely
//! across threads. This design allows multiple threads to log concurrently while maintaining
//! a consistent view of the accumulated logs.
//!
//! ## Integration with Global Logging
//!
//! The `InMemoryLogger` implements the `Logger` trait and can be used with the global
//! logging system via `add_global_logger()` or `set_global_loggers()`.

use crate::log_record::LogRecord;
use crate::logger::Logger;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;

/// An in-memory logger that stores log messages in a `Vec<String>`.
///
/// This logger captures all log records in memory, allowing you to retrieve and examine
/// them programmatically. It's particularly useful for testing scenarios where you need
/// to verify that certain log messages were produced, or in environments where normal
/// logging outputs are not available.
///
/// # Thread Safety
///
/// The `InMemoryLogger` is thread-safe and can be shared across multiple threads using
/// `Arc`. All operations on the internal log buffer are protected by a mutex.
///
/// # Example
///
/// ```rust
/// use logwise::InMemoryLogger;
/// use logwise::global_logger::{add_global_logger, set_global_loggers};
/// use std::sync::Arc;
///
/// // Create an in-memory logger
/// let logger = Arc::new(InMemoryLogger::new());
///
/// // Option 1: Add to existing loggers
/// add_global_logger(logger.clone());
///
/// // Option 2: Replace all loggers (useful for tests)
/// let logger = Arc::new(InMemoryLogger::new());
/// set_global_loggers(vec![logger.clone()]);
///
/// // Now logging will be captured in memory
/// logwise::info_sync!("Test message {value}", value=42);
///
/// // Retrieve the logs
/// let logs = logger.drain_logs();
/// assert!(logs.contains("Test message 42"));
/// ```
///
/// # Testing Example
///
/// ```rust
/// use logwise::InMemoryLogger;
/// use logwise::global_logger::set_global_loggers;
/// use std::sync::Arc;
///
/// fn function_under_test() {
///     logwise::warn_sync!("Something suspicious happened");
///     logwise::error_sync!("An error occurred: {code}", code=404);
/// }
///
/// # fn test_logging() {
/// // Set up in-memory logging for the test
/// let logger = Arc::new(InMemoryLogger::new());
/// set_global_loggers(vec![logger.clone()]);
///
/// // Run the function being tested
/// function_under_test();
///
/// // Verify the expected logs were produced
/// let logs = logger.drain_logs();
/// assert!(logs.contains("Something suspicious happened"));
/// assert!(logs.contains("An error occurred: 404"));
/// # }
/// ```
///
/// # Test Isolation Pattern
///
/// For better test isolation, you can save and restore the global loggers:
///
/// ```rust
/// use logwise::InMemoryLogger;
/// use logwise::global_logger::{global_loggers, set_global_loggers};
/// use std::sync::Arc;
///
/// # fn test_with_isolation() {
/// // Save the current global loggers
/// let original_loggers = global_loggers();
///
/// // Set up test-specific logging
/// let test_logger = Arc::new(InMemoryLogger::new());
/// set_global_loggers(vec![test_logger.clone()]);
///
/// // Run test code
/// logwise::info_sync!("Test-specific log message");
///
/// // Verify logs
/// let logs = test_logger.drain_logs();
/// assert!(logs.contains("Test-specific log message"));
///
/// // Restore original loggers
/// set_global_loggers(original_loggers);
/// # }
/// ```
#[derive(Debug)]
pub struct InMemoryLogger {
    logs: Mutex<Vec<String>>,
}

// ============================================================================
// BOILERPLATE TRAIT IMPLEMENTATIONS
// ============================================================================
//
// Design decisions for InMemoryLogger trait implementations:
//
// - Debug: Derived for diagnostic purposes and required by Logger trait
// - Default: Implemented with obvious zero-value (empty log buffer)
// - Clone: NOT implemented - expensive due to Mutex<Vec<String>>, and loggers
//   typically hold unique resources that shouldn't be duplicated
// - PartialEq/Eq: NOT implemented - equality semantics unclear for loggers,
//   and mutex state comparison is problematic
// - Hash: NOT implemented - requires Eq, and loggers shouldn't be hash keys
// - Display: NOT implemented - no meaningful display representation
// - Send/Sync: Automatically implemented due to Mutex usage (required for Logger trait)

impl Default for InMemoryLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryLogger {
    /// Creates a new `InMemoryLogger` with an empty log buffer.
    ///
    /// # Example
    ///
    /// ```rust
    /// use logwise::InMemoryLogger;
    ///
    /// let logger = InMemoryLogger::new();
    /// // Logger is ready to capture log messages
    /// ```
    pub fn new() -> Self {
        Self {
            logs: Mutex::new(Vec::new()),
        }
    }

    /// Drains all logs into a single string, clearing the internal buffer.
    ///
    /// This method retrieves all accumulated log messages, joins them with newlines,
    /// and returns them as a single string. The internal buffer is cleared after this
    /// operation, so subsequent calls will return an empty string unless new logs
    /// have been added.
    ///
    /// # Returns
    ///
    /// A string containing all log messages joined by newlines.
    ///
    /// # Example
    ///
    /// ```rust
    /// use logwise::InMemoryLogger;
    /// use logwise::global_logger::set_global_loggers;
    /// use std::sync::Arc;
    ///
    /// let logger = Arc::new(InMemoryLogger::new());
    /// set_global_loggers(vec![logger.clone()]);
    ///
    /// logwise::info_sync!("First message");
    /// logwise::warn_sync!("Second message");
    ///
    /// let logs = logger.drain_logs();
    /// assert!(logs.contains("First message"));
    /// assert!(logs.contains("Second message"));
    ///
    /// // Buffer is now empty
    /// let logs_again = logger.drain_logs();
    /// assert_eq!(logs_again, "");
    /// ```
    ///
    /// # Thread Safety
    ///
    /// This method is thread-safe and can be called from any thread. The internal
    /// mutex ensures that log retrieval and clearing are atomic operations.
    pub fn drain_logs(&self) -> String {
        let mut logs = self.logs.lock().unwrap();
        let result = logs.join("\n");
        logs.clear();
        result
    }

    /// Flushes all logs to the console, clearing the internal buffer.
    ///
    /// This method outputs all accumulated log messages to stderr (or console.log
    /// in WASM environments) and then clears the internal buffer. This is useful
    /// for debugging when you want to see the logs immediately without retrieving
    /// them programmatically.
    ///
    /// # Platform Behavior
    ///
    /// - On native platforms: Logs are written to stderr using `eprintln!`
    /// - On WASM: Logs are written using `web_sys::console::log_1`
    ///
    /// # Example
    ///
    /// ```rust
    /// use logwise::InMemoryLogger;
    /// use logwise::global_logger::set_global_loggers;
    /// use std::sync::Arc;
    ///
    /// let logger = Arc::new(InMemoryLogger::new());
    /// set_global_loggers(vec![logger.clone()]);
    ///
    /// logwise::error_sync!("Critical error occurred!");
    ///
    /// // Output logs to console for immediate debugging
    /// logger.drain_to_console();
    /// // The error message is now printed to stderr/console
    /// ```
    pub fn drain_to_console(&self) {
        let mut logs = self.logs.lock().unwrap();
        for log in logs.iter() {
            #[cfg(target_arch = "wasm32")]
            web_sys::console::log_1(&log.clone().into());
            #[cfg(not(target_arch = "wasm32"))]
            eprintln!("{}", log);
        }
        logs.clear();
    }

    /// Creates a future that periodically drains logs to the console.
    ///
    /// This method returns a future that will periodically call [`drain_to_console`](Self::drain_to_console)
    /// at approximately 100ms intervals for the specified duration. This is particularly
    /// useful in adversarial async environments where:
    ///
    /// - Console access is restricted to a single thread
    /// - You need to interleave logging with other async operations
    /// - You're working in WASM where direct console access may be limited
    ///
    /// The future implements a cooperative pattern that yields control between drain
    /// operations, allowing other futures to make progress.
    ///
    /// # Parameters
    ///
    /// - `duration`: How long the periodic draining should continue
    ///
    /// # Important Notes
    ///
    /// - The draining happens approximately every 100ms
    /// - Other futures must yield control for the draining to occur
    /// - The future completes after the specified duration
    /// - A warning is logged when the task completes
    ///
    /// # Example
    ///
    /// ```
    /// use logwise::InMemoryLogger;
    /// use logwise::global_logger::set_global_loggers;
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// # async fn example() {
    /// let logger = Arc::new(InMemoryLogger::new());
    /// set_global_loggers(vec![logger.clone()]);
    ///
    /// // Start periodic draining for 5 seconds
    /// let drain_task = logger.periodic_drain_to_console(Duration::from_secs(5));
    ///
    /// // The drain task can be combined with other async work using
    /// // your async runtime's combinators (e.g., select!, join!, etc.)
    /// // Here's a conceptual example:
    ///
    /// // Run the drain task - it will periodically output logs to console
    /// // drain_task.await;
    ///
    /// // In practice, you'd run this alongside your actual async work:
    /// // futures::select! {
    /// //     _ = drain_task => println!("Draining completed"),
    /// //     _ = your_async_work() => println!("Work completed"),
    /// // }
    /// # }
    /// ```
    pub fn periodic_drain_to_console(
        self: &Arc<Self>,
        duration: crate::Duration,
    ) -> impl Future<Output = ()> {
        PeriodicDrainToConsole {
            logger: self.clone(),
            duration,
            start_time: None,
        }
    }
}

/// Implementation of the `Logger` trait for `InMemoryLogger`.
///
/// This implementation captures all log records in memory by converting them
/// to strings and storing them in the internal vector. Both synchronous and
/// asynchronous logging are supported, with the async version being a simple
/// wrapper around the synchronous implementation.
impl Logger for InMemoryLogger {
    /// Processes and stores a log record in the internal buffer.
    ///
    /// The record is converted to a string using its `Display` implementation
    /// and added to the internal vector of logs.
    fn finish_log_record(&self, record: LogRecord) {
        let log_string = record.to_string();
        let mut logs = self.logs.lock().unwrap();
        logs.push(log_string);
    }

    /// Asynchronously processes and stores a log record.
    ///
    /// This is a simple async wrapper around the synchronous implementation,
    /// as in-memory storage doesn't require actual async I/O operations.
    fn finish_log_record_async<'s>(
        &'s self,
        record: LogRecord,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 's>> {
        // Simple async wrapper around the synchronous implementation
        Box::pin(async move {
            self.finish_log_record(record);
        })
    }

    /// No-op for in-memory logger.
    ///
    /// Since logs are stored in memory and not written to external resources,
    /// there's nothing to flush when the logger is being shut down.
    fn prepare_to_die(&self) {
        // No-op since we're storing in memory, no flushing needed
    }
}

/// Future that periodically drains logs to the console.
///
/// This future is created by [`InMemoryLogger::periodic_drain_to_console`] and
/// implements a cooperative async pattern for draining logs at regular intervals.
/// It's designed to work in environments where console access might be restricted
/// or where you need to interleave logging operations with other async work.
struct PeriodicDrainToConsole {
    logger: Arc<InMemoryLogger>,
    duration: Duration,
    start_time: Option<crate::sys::Instant>,
}

impl Future for PeriodicDrainToConsole {
    type Output = ();

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        // Drain logs to console

        let start_time = match self.start_time {
            Some(t) => t,
            None => {
                // Initialize start time if not set
                let now = crate::sys::Instant::now();
                self.start_time = Some(now);
                now
            }
        };
        let finished = start_time.elapsed() >= self.duration;
        if finished {
            logwise::warn_sync!(
                "PeriodicDrainToConsole task completing; you will need a new task to see logs after this point."
            );
        }
        self.logger.drain_to_console();

        if finished {
            // If the duration has elapsed, finish the future
            Poll::Ready(())
        } else {
            //empirically, if we do this inline in chrome, we will repeatedly poll the future
            //instead of interleaving with other tasks.
            #[cfg(not(target_arch = "wasm32"))]
            use std::thread;
            #[cfg(target_arch = "wasm32")]
            use wasm_thread as thread;
            // Otherwise, register the waker and continue polling
            let move_waker = cx.waker().clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(100));
                move_waker.wake();
            });
            Poll::Pending
        }
    }
}
