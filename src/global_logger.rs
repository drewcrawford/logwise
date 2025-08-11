//SPDX-License-Identifier: MIT OR Apache-2.0
use crate::logger::Logger;
use crate::stderror_logger::StdErrorLogger;
use std::sync::{Arc, OnceLock};
use logwise::spinlock::Spinlock;

/**
The global loggers.

 */
static GLOBAL_LOGGERS_PTR: OnceLock<Spinlock<Vec<Arc<dyn Logger>>>> = OnceLock::new();

/**
Gets the current global loggers.
Returns a Vec of Arcs to ensure the loggers stay alive during the logging operation.
*/
pub fn global_loggers() -> Vec<Arc<dyn Logger>> {
    GLOBAL_LOGGERS_PTR.get_or_init(|| {
        // Initialize the global loggers with a default StdErrorLogger.
        Spinlock::new(vec![Arc::new(StdErrorLogger::new())])
    }).with(|loggers| loggers.clone())
}

/**
Adds a logger to the global loggers.

The logger will be wrapped in Arc and appended to the list. This function is thread-safe.
*/
pub fn add_global_logger(logger: Arc<dyn Logger>) {
    let logger_clone = logger.clone();
    GLOBAL_LOGGERS_PTR.get_or_init(|| {
        // Initialize the global loggers with a default StdErrorLogger.
        Spinlock::new(vec![Arc::new(StdErrorLogger::new())])
    }).with_mut(|loggers| loggers.push(logger));
}

/**
Sets the global loggers to a new set of implementations.

This replaces all existing loggers. This function is thread-safe.
Previous loggers will be properly dropped when no longer referenced.
*/
pub fn set_global_loggers(new_loggers: Vec<Arc<dyn Logger>>) {
    let loggers_clone = new_loggers.clone();
    GLOBAL_LOGGERS_PTR.get_or_init(|| {
        // Initialize the global loggers with the provided loggers.
        Spinlock::new(loggers_clone)
    }).with_mut(|loggers| *loggers = new_loggers);
}