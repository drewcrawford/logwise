//SPDX-License-Identifier: MIT OR Apache-2.0
use crate::logger::Logger;
use crate::stderror_logger::StdErrorLogger;
use std::sync::{Arc, Mutex, OnceLock};



/**
The global logger.

 */
static GLOBAL_LOGGER_PTR: OnceLock<Mutex<Arc<dyn Logger>>> = OnceLock::new();

/**
Gets the current global logger.
Returns an Arc to ensure the logger stays alive during the logging operation.
*/
pub fn global_logger() -> Arc<dyn Logger> {
    GLOBAL_LOGGER_PTR.get_or_init(|| {
        // Initialize the global logger with a default logger.
        Mutex::new(Arc::new(StdErrorLogger::new()))
    }).lock().unwrap().clone()
}

/**
Sets the global logger to a new implementation.

The logger will be wrapped in Arc and stored. This function is thread-safe.
Previous loggers will be properly dropped when no longer referenced.
*/
pub fn set_global_logger(logger: Arc<dyn Logger>) {
    let arc_convert_clone = logger.clone();
    *GLOBAL_LOGGER_PTR.get_or_init(|| {
        // Initialize the global logger with a default logger.
        Mutex::new(arc_convert_clone)
    }).lock().unwrap() = logger;
}