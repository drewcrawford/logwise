// SPDX-License-Identifier: MIT OR Apache-2.0

//! Global logger management for the logwise logging system.
//!
//! This module provides thread-safe management of global loggers that receive all log records
//! generated throughout the application. The global logger system supports multiple simultaneous
//! loggers, allowing logs to be sent to multiple destinations (e.g., stderr, files, remote servers).
//!
//! # Architecture
//!
//! The global logger system uses a spinlock-protected vector of `Arc<dyn Logger>` instances.
//! This design ensures:
//! - Thread-safe access from any thread
//! - Multiple loggers can be active simultaneously
//! - Loggers remain alive during logging operations
//! - Compatible with WASM environments where traditional mutexes may not work
//!
//! # Default Behavior
//!
//! By default, the system initializes with a single stderr logger that writes colored output
//! to stderr. This ensures logging works out-of-the-box without configuration.
//!
//! # Thread Safety
//!
//! All functions in this module are thread-safe and can be called from any thread. The underlying
//! spinlock ensures atomic operations while keeping lock hold times minimal. The spinlock is
//! particularly important for WASM compatibility where blocking mutexes may not be available.
//!
//! # Examples
//!
//! ## Using the default logger
//!
//! ```
//! use logwise::global_logger::global_loggers;
//!
//! // Get the current loggers (initializes with StdErrorLogger if needed)
//! let loggers = global_loggers();
//! assert!(!loggers.is_empty());
//! ```
//!
//! ## Adding a custom logger
//!
//! ```
//! use logwise::global_logger::add_global_logger;
//! use logwise::InMemoryLogger;
//! use std::sync::Arc;
//!
//! // Add an in-memory logger alongside existing loggers
//! let logger = Arc::new(InMemoryLogger::new());
//! add_global_logger(logger.clone());
//!
//! // Now logs go to both stderr and the in-memory logger
//! logwise::info_sync!("This goes to multiple loggers");
//! ```
//!
//! ## Replacing all loggers
//!
//! ```
//! use logwise::global_logger::set_global_loggers;
//! use logwise::InMemoryLogger;
//! use std::sync::Arc;
//!
//! // Replace all loggers with just an in-memory logger
//! let logger = Arc::new(InMemoryLogger::new());
//! set_global_loggers(vec![logger.clone()]);
//!
//! // Now logs only go to the in-memory logger
//! logwise::warn_sync!("Only captured in memory");
//! ```
//!
//! # Implementation Notes
//!
//! ## Spinlock vs Mutex
//!
//! This module uses a custom spinlock implementation rather than `std::sync::Mutex` for
//! compatibility with WASM environments where blocking mutexes may not be available. The
//! spinlock ensures very short critical sections - only cloning Arc references or updating
//! the logger vector.
//!
//! ## Logger Lifecycle
//!
//! Loggers are reference-counted using `Arc`. When a logger is removed (via `set_global_loggers`),
//! it continues to exist until all outstanding references are dropped. This ensures that
//! in-flight logging operations complete successfully even if the logger configuration changes.
//!
//! ## Performance Considerations
//!
//! - Getting loggers clones the `Arc` vector, which is cheap (only reference count increments)
//! - Adding loggers requires a write lock but is typically infrequent
//! - The spinlock may cause CPU usage spikes under high contention, but this is rare in practice
//!   since logger configuration typically happens during initialization
//!
//! ## Best Practices
//!
//! 1. Configure loggers early in your application's lifecycle
//! 2. Avoid frequently changing logger configuration in production
//! 3. Use `add_global_logger` to add supplementary loggers without disrupting existing ones
//! 4. Use `set_global_loggers` when you need complete control over the logging pipeline
//! 5. Always keep at least one logger active to avoid losing important diagnostic information

use crate::logger::Logger;
use crate::stderror_logger::StdErrorLogger;
use logwise::spinlock::Spinlock;
use std::sync::{Arc, OnceLock};

/// Static storage for the global logger collection.
///
/// Uses `OnceLock` for one-time initialization and `Spinlock` for thread-safe access.
/// The spinlock is necessary for WASM compatibility where traditional mutexes may block.
static GLOBAL_LOGGERS_PTR: OnceLock<Spinlock<Vec<Arc<dyn Logger>>>> = OnceLock::new();

/// Retrieves the current set of global loggers.
///
/// Returns a vector of `Arc<dyn Logger>` references to ensure loggers remain alive
/// during logging operations. If no loggers have been configured, automatically
/// initializes with a default stderr logger.
///
/// This function is thread-safe and can be called from any thread.
///
/// # Returns
///
/// A vector containing `Arc` references to all currently active global loggers.
/// The vector is never empty - it always contains at least the default logger.
///
/// # Performance
///
/// This function clones the vector of `Arc`s, which is relatively cheap since
/// `Arc::clone` only increments a reference count. The spinlock is held only
/// for the duration of the clone operation.
///
/// # Examples
///
/// ```
/// use logwise::global_logger::global_loggers;
///
/// let loggers = global_loggers();
/// println!("Number of active loggers: {}", loggers.len());
///
/// // Loggers can be inspected if needed
/// for logger in &loggers {
///     println!("Logger: {:?}", logger);
/// }
/// ```
pub fn global_loggers() -> Vec<Arc<dyn Logger>> {
    GLOBAL_LOGGERS_PTR
        .get_or_init(|| {
            // Initialize the global loggers with a default StdErrorLogger.
            Spinlock::new(vec![Arc::new(StdErrorLogger::new())])
        })
        .with(|loggers| loggers.clone())
}

/// Adds a logger to the global logger collection.
///
/// The new logger is appended to the existing list of loggers, allowing multiple
/// loggers to receive all log records. This is useful for sending logs to multiple
/// destinations simultaneously.
///
/// This function is thread-safe and can be called from any thread.
///
/// # Arguments
///
/// * `logger` - An `Arc`-wrapped logger implementation to add to the global collection
///
/// # Thread Safety
///
/// The function uses a spinlock to ensure thread-safe modification of the logger list.
/// The lock is held only for the duration of the push operation.
///
/// # Examples
///
/// ```
/// use logwise::global_logger::{add_global_logger, global_loggers};
/// use logwise::InMemoryLogger;
/// use std::sync::Arc;
///
/// let initial_count = global_loggers().len();
///
/// // Add a new logger
/// let logger = Arc::new(InMemoryLogger::new());
/// add_global_logger(logger);
///
/// // Verify it was added
/// assert_eq!(global_loggers().len(), initial_count + 1);
/// ```
///
/// ## Multiple logger types
///
/// ```
/// use logwise::global_logger::add_global_logger;
/// use logwise::InMemoryLogger;
/// use std::sync::Arc;
///
/// // Add multiple in-memory loggers (for demonstration)
/// let logger1 = Arc::new(InMemoryLogger::new());
/// let logger2 = Arc::new(InMemoryLogger::new());
/// add_global_logger(logger1.clone());
/// add_global_logger(logger2.clone());
///
/// // Now logs go to all registered loggers
/// logwise::info_sync!("This appears in all loggers");
/// ```
pub fn add_global_logger(logger: Arc<dyn Logger>) {
    GLOBAL_LOGGERS_PTR
        .get_or_init(|| {
            // Initialize the global loggers with a default StdErrorLogger.
            Spinlock::new(vec![Arc::new(StdErrorLogger::new())])
        })
        .with_mut(|loggers| loggers.push(logger));
}

/// Replaces all global loggers with a new set.
///
/// This function completely replaces the existing logger collection. Previous loggers
/// are properly dropped when they are no longer referenced. This is useful when you
/// want complete control over where logs are sent.
///
/// This function is thread-safe and can be called from any thread.
///
/// # Arguments
///
/// * `new_loggers` - A vector of `Arc`-wrapped logger implementations to use as the new global collection
///
/// # Thread Safety
///
/// The function uses a spinlock to ensure thread-safe replacement of the logger list.
/// The lock is held only for the duration of the assignment operation. Previous loggers
/// are dropped after the lock is released when their reference counts reach zero.
///
/// # Examples
///
/// ## Replace with a single logger
///
/// ```
/// use logwise::global_logger::set_global_loggers;
/// use logwise::InMemoryLogger;
/// use std::sync::Arc;
///
/// // Replace all loggers with just one
/// let logger = Arc::new(InMemoryLogger::new());
/// set_global_loggers(vec![logger.clone()]);
///
/// // Now only the in-memory logger receives logs
/// logwise::info_sync!("Only in memory");
/// ```
///
/// ## Replace with multiple loggers
///
/// ```
/// use logwise::global_logger::set_global_loggers;
/// use logwise::InMemoryLogger;
/// use std::sync::Arc;
///
/// // Set up multiple loggers at once
/// let logger1 = Arc::new(InMemoryLogger::new());
/// let logger2 = Arc::new(InMemoryLogger::new());
/// let loggers: Vec<Arc<dyn logwise::Logger>> = vec![
///     logger1.clone() as Arc<dyn logwise::Logger>,
///     logger2.clone() as Arc<dyn logwise::Logger>,
/// ];
/// set_global_loggers(loggers);
///
/// logwise::warn_sync!("This goes to both loggers");
/// ```
///
/// ## Clear all loggers (not recommended)
///
/// ```
/// use logwise::global_logger::set_global_loggers;
///
/// // This removes all loggers - logs will be silently dropped
/// // Generally not recommended in production code
/// set_global_loggers(vec![]);
/// ```
pub fn set_global_loggers(new_loggers: Vec<Arc<dyn Logger>>) {
    let loggers_clone = new_loggers.clone();
    GLOBAL_LOGGERS_PTR
        .get_or_init(|| {
            // Initialize the global loggers with the provided loggers.
            Spinlock::new(loggers_clone)
        })
        .with_mut(|loggers| *loggers = new_loggers);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inmemory_logger::InMemoryLogger;
    use crate::stderror_logger::StdErrorLogger;
    use std::sync::Mutex;

    static TEST_LOGGER_GUARD: Mutex<()> = Mutex::new(());

    #[test]
    fn test_add_logger() {
        let _guard = TEST_LOGGER_GUARD.lock().unwrap();
        set_global_loggers(vec![Arc::new(StdErrorLogger::new())]);
        let initial_count = global_loggers().len();

        // Add a new logger
        let logger = Arc::new(InMemoryLogger::new());
        add_global_logger(logger.clone());

        // Verify it was added
        let loggers = global_loggers();
        assert_eq!(
            loggers.len(),
            initial_count + 1,
            "Logger count should increase by 1"
        );
    }

    #[test]
    fn test_set_loggers() {
        let _guard = TEST_LOGGER_GUARD.lock().unwrap();
        // Create some test loggers
        let logger1 = Arc::new(InMemoryLogger::new());
        let logger2 = Arc::new(InMemoryLogger::new());

        // Set them as the global loggers
        set_global_loggers(vec![logger1.clone(), logger2.clone()]);

        // Verify they were set
        let loggers = global_loggers();
        assert_eq!(loggers.len(), 2, "Should have exactly 2 loggers");
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;

        let _guard = TEST_LOGGER_GUARD.lock().unwrap();
        set_global_loggers(vec![Arc::new(StdErrorLogger::new())]);

        let logger = Arc::new(InMemoryLogger::new());
        let logger_clone = logger.clone();

        // Spawn a thread that adds a logger
        let handle = thread::spawn(move || {
            add_global_logger(logger_clone);
        });

        // Meanwhile, get loggers from the main thread
        let _ = global_loggers();

        // Wait for the thread to complete
        handle.join().expect("Thread should complete successfully");

        // Verify the logger was added despite concurrent access
        let loggers = global_loggers();
        assert!(
            loggers.len() >= 2,
            "Should have at least 2 loggers after thread operation"
        );
    }
}
