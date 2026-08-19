// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deadline monitoring: [`heartbeat`] hands back an RAII guard that emits a `PerfWarn`
//! record if the work it brackets outlives its duration.
//!
//! Unlike a perfwarn interval, which can only report once the region ends, heartbeats are
//! watched by one lazily spawned background thread (a `wasm_lite_std` worker on wasm32)
//! that blocks on the nearest deadline, so a hang is reported while it is still happening
//! and again at the drop site if the guard was already late. Each guard captures
//! [`Context::current`] and its caller location so warnings are attributed to the
//! originating task, and degrades to an inert, channel-less guard when `PerfWarn` is off.

use crate::context::Context;
use crate::global_logger::global_loggers;
use crate::log_record::LogRecord;
use crate::sys::{Duration, Instant};
use crate::{Level, log_enabled};
use std::collections::HashMap;
use std::panic::Location;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use wasm_lite_std::mpsc;

#[derive(Clone, Copy, Debug)]
struct CallSite {
    file: &'static str,
    line: u32,
    column: u32,
}

impl CallSite {
    #[track_caller]
    fn new() -> Self {
        let location = Location::caller();
        Self {
            file: location.file(),
            line: location.line(),
            column: location.column(),
        }
    }
}

#[derive(Debug)]
struct Heartbeat {
    id: u64,
    name: &'static str,
    created_at: Instant,
    deadline: Instant,
    callsite: CallSite,
    warned: bool,
    context: Context,
}

enum Message {
    Register(Heartbeat),
    Complete(u64),
}

static CHANNEL: OnceLock<mpsc::Sender<Message>> = OnceLock::new();
static HEARTBEAT_ID: AtomicU64 = AtomicU64::new(1);

fn channel() -> mpsc::Sender<Message> {
    CHANNEL
        .get_or_init(|| {
            let (tx, rx) = mpsc::channel();
            spawn_watcher(rx);
            tx
        })
        .clone()
}

/// RAII guard that ensures work completes before a specified deadline.
///
/// `HeartbeatGuard` monitors the duration of an operation and logs a warning if
/// it exceeds the specified deadline. This is useful for detecting operations that
/// are taking longer than expected, which might indicate performance issues or
/// deadlocks.
///
/// # Behavior
///
/// - When created, the guard registers with a background watcher thread
/// - If the deadline is exceeded while the guard exists, a warning is logged immediately
/// - If the guard is dropped after the deadline, another warning is logged at the drop site
/// - If the guard is dropped before the deadline, no warning is logged
///
/// # Usage
///
/// Create via the [`heartbeat`] function rather than calling `new` directly:
///
/// ```rust
/// use std::time::Duration;
///
/// fn critical_operation() {
///     // Warn if this operation takes more than 5 seconds
///     let _guard = logwise_runtime::heartbeat("database_sync", Duration::from_secs(5));
///
///     // ... perform the operation ...
///     // If it takes > 5 seconds, a warning is logged
/// }
/// ```
///
/// # Thread Safety
///
/// The guard communicates with a background watcher thread via message passing.
/// It is safe to create guards from multiple threads simultaneously.
///
/// # Performance
///
/// Guard creation is cheap (a few atomic operations and a message send).
/// The background watcher thread handles deadline monitoring efficiently using
/// timeouts rather than polling.
#[derive(Debug)]
pub struct HeartbeatGuard {
    id: u64,
    name: &'static str,
    created_at: Instant,
    deadline: Instant,
    callsite: CallSite,
    sender: Option<mpsc::Sender<Message>>,
    context: Context,
}

impl HeartbeatGuard {
    /// Creates a new `HeartbeatGuard` that will warn if not dropped within the given duration.
    ///
    /// # Arguments
    ///
    /// * `name` - A static string identifying this heartbeat in log messages
    /// * `duration` - The maximum allowed duration before a warning is logged
    ///
    /// # Example
    ///
    /// On `wasm32`, this doctest configures wasm-bindgen to run in a browser
    /// because thread spawning is not yet supported under Node.
    ///
    /// ```rust
    /// use std::time::Duration;
    /// use logwise_runtime::HeartbeatGuard;
    ///
    /// let guard = HeartbeatGuard::new("network_request", Duration::from_secs(30));
    /// // ... perform network operation ...
    /// drop(guard); // Logs warning if > 30 seconds elapsed
    /// ```
    ///
    /// # Note
    ///
    /// Prefer using the [`heartbeat`] function for convenience:
    ///
    /// ```rust
    /// use std::time::Duration;
    ///
    /// let guard = logwise_runtime::heartbeat("operation", Duration::from_secs(5));
    /// ```
    #[track_caller]
    pub fn new(name: &'static str, duration: Duration) -> Self {
        if !log_enabled!(Level::PerfWarn) {
            let now = Instant::now();
            return Self {
                id: 0,
                name,
                created_at: now,
                deadline: now,
                callsite: CallSite::new(),
                sender: None,
                context: Context::current(),
            };
        }

        let id = HEARTBEAT_ID.fetch_add(1, Ordering::Relaxed);
        let created_at = Instant::now();
        let deadline = created_at + duration;
        let callsite = CallSite::new();
        let context = Context::current();
        let sender = channel();

        let heartbeat = Heartbeat {
            id,
            name,
            created_at,
            deadline,
            callsite,
            warned: false,
            context: context.clone(),
        };

        // Ignore send failures so the guard remains cheap even during shutdown.
        let _ = sender.send_sync(Message::Register(heartbeat));

        Self {
            id,
            name,
            created_at,
            deadline,
            callsite,
            sender: Some(sender),
            context,
        }
    }
}

impl Drop for HeartbeatGuard {
    #[track_caller]
    fn drop(&mut self) {
        let Some(sender) = self.sender.as_ref() else {
            return;
        };

        let now = Instant::now();
        if now > self.deadline {
            let drop_site = CallSite::new();
            log_heartbeat(&self.context, drop_site, |record| {
                record.log_owned(format!(
                    "heartbeat \"{}\" dropped after deadline by {:?} ",
                    self.name,
                    now.duration_since(self.deadline)
                ));
                record.log_owned(format!(
                    "(created at {}:{}:{}, {:?} ago) ",
                    self.callsite.file,
                    self.callsite.line,
                    self.callsite.column,
                    now.duration_since(self.created_at)
                ));
            });
        }

        let _ = sender.send_sync(Message::Complete(self.id));
    }
}

fn log_heartbeat(context: &Context, callsite: CallSite, write: impl FnOnce(&mut LogRecord)) {
    let mut record = LogRecord::new(Level::PerfWarn);
    context._log_prelude(&mut record);

    record.log("PERFWARN: HEARTBEAT ");
    record.log(callsite.file);
    record.log_owned(format!(":{}:{} ", callsite.line, callsite.column));
    record.log_timestamp();
    write(&mut record);

    let loggers = global_loggers();
    for logger in loggers {
        logger.finish_log_record(record.clone_for_logger(logger.as_ref()));
    }
}

fn spawn_watcher(receiver: mpsc::Receiver<Message>) {
    #[cfg(not(target_arch = "wasm32"))]
    let _ = std::thread::Builder::new()
        .name("logwise-heartbeat".to_string())
        .spawn(move || heartbeat_loop(receiver));

    #[cfg(target_arch = "wasm32")]
    let _ = wasm_lite_std::spawn(move || heartbeat_loop(receiver));
}

fn heartbeat_loop(receiver: mpsc::Receiver<Message>) {
    let mut heartbeats: HashMap<u64, Heartbeat> = HashMap::new();
    loop {
        let next_deadline = heartbeats
            .values()
            .filter(|hb| !hb.warned)
            .map(|hb| hb.deadline)
            .min();

        //with no pending deadline, block until a message arrives rather than
        //waking every 250ms forever
        let received = match next_deadline {
            Some(deadline) => receiver.recv_sync_timeout(deadline),
            None => receiver
                .recv_sync()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
        };

        match received {
            Ok(Message::Register(hb)) => {
                heartbeats.insert(hb.id, hb);
            }
            Ok(Message::Complete(id)) => {
                if let Some(hb) = heartbeats.remove(&id) {
                    let now = Instant::now();
                    if !hb.warned && now >= hb.deadline {
                        log_heartbeat(&hb.context, hb.callsite, |record| {
                            record.log_owned(format!(
                                "heartbeat \"{}\" missed deadline by {:?} ",
                                hb.name,
                                now.duration_since(hb.deadline)
                            ));
                            record.log_owned(format!(
                                "(created {:?} ago) ",
                                now.duration_since(hb.created_at)
                            ));
                        });
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => { /* fall through to deadline check */ }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(other) => panic!("unexpected message: {:?}", other),
        }

        let now = Instant::now();
        for heartbeat in heartbeats.values_mut() {
            if !heartbeat.warned && now >= heartbeat.deadline {
                log_heartbeat(&heartbeat.context, heartbeat.callsite, |record| {
                    record.log_owned(format!(
                        "heartbeat \"{}\" missed deadline by {:?} ",
                        heartbeat.name,
                        now.duration_since(heartbeat.deadline)
                    ));
                    record.log_owned(format!(
                        "(created {:?} ago) ",
                        now.duration_since(heartbeat.created_at)
                    ));
                });
                heartbeat.warned = true;
            }
        }
    }
}

/// Creates a [`HeartbeatGuard`] that will warn if it is not dropped before `duration`.
///
/// This function creates a deadline monitor for an operation. If the returned guard
/// is not dropped before the specified duration elapses, a performance warning is
/// logged. This is useful for detecting operations that are taking longer than expected.
///
/// # Arguments
///
/// * `name` - A static string identifying this heartbeat in log messages
/// * `duration` - The maximum allowed duration before a warning is logged
///
/// # Returns
///
/// A [`HeartbeatGuard`] that monitors the deadline. Keep this guard alive for the
/// duration of the operation you want to monitor.
///
/// # Example
///
/// ```rust
/// use std::time::Duration;
///
/// fn fetch_user_data() {
///     // Warn if fetching takes more than 2 seconds
///     let _guard = logwise_runtime::heartbeat("fetch_user_data", Duration::from_secs(2));
///
///     // ... fetch data from database or network ...
/// }
/// // When _guard is dropped, if > 2 seconds elapsed, a warning is logged
/// ```
///
/// # Multiple Heartbeats
///
/// You can have multiple heartbeats active simultaneously:
///
/// ```rust
/// use std::time::Duration;
///
/// fn complex_operation() {
///     let _outer = logwise_runtime::heartbeat("complex_operation", Duration::from_secs(30));
///
///     {
///         let _phase1 = logwise_runtime::heartbeat("phase1", Duration::from_secs(10));
///         // ... phase 1 work ...
///     }
///
///     {
///         let _phase2 = logwise_runtime::heartbeat("phase2", Duration::from_secs(15));
///         // ... phase 2 work ...
///     }
/// }
/// ```
#[track_caller]
pub fn heartbeat(name: &'static str, duration: Duration) -> HeartbeatGuard {
    HeartbeatGuard::new(name, duration)
}

#[cfg(test)]
mod tests {
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test)]
    fn assert_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<super::HeartbeatGuard>();
    }
}
