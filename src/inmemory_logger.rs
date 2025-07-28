//SPDX-License-Identifier: MIT OR Apache-2.0
use std::sync::{Arc, Mutex};
use crate::logger::Logger;
use crate::log_record::LogRecord;
use std::pin::Pin;
use std::future::Future;
use std::task::Poll;
use std::time::Duration;

/**
An in-memory logger that stores log messages in a Vec<String>.

This logger is designed for cases where logging might not otherwise be available,
such as when tools redirect stderr incorrectly, during testing, or when you need
to capture and examine log output programmatically.

# Example

```rust
use logwise::InMemoryLogger;
use logwise::global_logger::{add_global_logger, set_global_loggers};
use std::sync::Arc;

// Add an in-memory logger to the existing loggers
let logger = Arc::new(InMemoryLogger::new());
add_global_logger(logger.clone());

// Or replace all loggers with just the in-memory logger
let logger = Arc::new(InMemoryLogger::new());
set_global_loggers(vec![logger.clone()]);

// Use logging normally
// logwise::info_sync!("Hello {world}!", world="logwise");
// logwise::warn_sync!("This is a warning");

```
*/
#[derive(Debug)]
pub struct InMemoryLogger {
    logs: Mutex<Vec<String>>,
}

impl InMemoryLogger {
    /**
    Creates a new InMemoryLogger with an empty log buffer.
    */
    pub fn new() -> Self {
        Self {
            logs: Mutex::new(Vec::new()),
        }
    }

    /**
    Drains all logs into a single string, clearing the internal buffer.
    
    Returns a string with all log messages joined by newlines.
    After calling this method, the internal log buffer will be empty.
    */
    pub fn drain_logs(&self) -> String {
        let mut logs = self.logs.lock().unwrap();
        let result = logs.join("\n");
        logs.clear();
        result
    }

    /**
    Flushes all logs to the console.

    After calling this method, the internal log buffer will be empty.
    */
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

    /**

    A task that periodically drains logs to the console.

    This future implements a "stoopid loop"™️ that periodically calls [drain_to_console].

    # Discussion

    The idea here is you may be in an adversarial environment.  For example, suppose that console
    is only accessible from a single thread, so all log messages must be dumped on the thread.

    Further suppose that the thread you must use needs to be also doing something, like the thing
    you are trying to log about.  In this case you may want to weave the log draining op
    with your own operations.  Note that this is subject to the usual caveats of async programming,
    namely that the other future must yield control to allow the log draining to happen.
    */
    pub fn periodic_drain_to_console(self: &Arc<Self>, duration: crate::Duration) -> impl Future<Output=()> {
        PeriodicDrainToConsole {
            logger: self.clone(),
            duration: duration.into(),
            start_time: None,
        }
    }

}

impl Logger for InMemoryLogger {
    fn finish_log_record(&self, record: LogRecord) {
        let log_string = record.to_string();
        let mut logs = self.logs.lock().unwrap();
        logs.push(log_string);
    }

    fn finish_log_record_async<'s>(&'s self, record: LogRecord) -> Pin<Box<dyn Future<Output=()> + Send + 's>> {
        // Simple async wrapper around the synchronous implementation
        Box::pin(async move {
            self.finish_log_record(record);
        })
    }

    fn prepare_to_die(&self) {
        // No-op since we're storing in memory, no flushing needed
    }
}

struct PeriodicDrainToConsole {
    logger: Arc<InMemoryLogger>,
    duration: Duration,
    start_time: Option<crate::sys::Instant>,
}

impl Future for PeriodicDrainToConsole {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
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
            logwise::warn_sync!("PeriodicDrainToConsole task completing; you will need a new task to see logs after this point.");
        }
        self.logger.drain_to_console();

        if finished {
            // If the duration has elapsed, finish the future
            Poll::Ready(())
        }
        else {
            //empirically, if we do this inline in chrome, we will repeatedly poll the future
            //instead of interleaving with other tasks.
            #[cfg(target_arch = "wasm32")]
            use wasm_thread as thread;
            #[cfg(not(target_arch = "wasm32"))]
            use std::thread;
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