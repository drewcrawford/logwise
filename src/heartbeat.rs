// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::context::Context;
use crate::global_logger::global_loggers;
use crate::log_record::LogRecord;
use crate::sys::{Duration, Instant};
use crate::{Level, log_enabled};
use std::collections::HashMap;
use std::panic::Location;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use wasm_safe_mutex::mpsc;

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
/// Create via [`heartbeat`] to register the deadline with the background watcher.
/// Dropping on time is silent; missing a deadline logs `perfwarn` warnings both
/// when the deadline is crossed and when the guard is eventually dropped.
#[derive(Debug)]
pub struct HeartbeatGuard {
    id: u64,
    name: &'static str,
    created_at: Instant,
    deadline: Instant,
    callsite: CallSite,
    sender: Option<mpsc::Sender<Message>>,
}

impl HeartbeatGuard {
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
            };
        }

        let id = HEARTBEAT_ID.fetch_add(1, Ordering::Relaxed);
        let created_at = Instant::now();
        let deadline = created_at + duration;
        let callsite = CallSite::new();
        let sender = channel();

        let heartbeat = Heartbeat {
            id,
            name,
            created_at,
            deadline,
            callsite,
            warned: false,
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
            log_heartbeat(drop_site, |record| {
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

fn log_heartbeat(callsite: CallSite, write: impl FnOnce(&mut LogRecord)) {
    let mut record = LogRecord::new(Level::PerfWarn);
    let ctx = Context::current();
    ctx._log_prelude(&mut record);

    record.log("PERFWARN: HEARTBEAT ");
    record.log(callsite.file);
    record.log_owned(format!(":{}:{} ", callsite.line, callsite.column));
    record.log_timestamp();
    write(&mut record);

    let loggers = global_loggers();
    for logger in loggers {
        logger.finish_log_record(record.clone());
    }
}

fn spawn_watcher(receiver: mpsc::Receiver<Message>) {
    #[cfg(not(target_arch = "wasm32"))]
    let _ = std::thread::Builder::new()
        .name("logwise-heartbeat".to_string())
        .spawn(move || heartbeat_loop(receiver));

    #[cfg(target_arch = "wasm32")]
    let _ = wasm_thread::spawn(move || heartbeat_loop(receiver));
}

fn heartbeat_loop(receiver: mpsc::Receiver<Message>) {
    let mut heartbeats: HashMap<u64, Heartbeat> = HashMap::new();
    loop {
        let now = Instant::now();
        let next_deadline = heartbeats
            .values()
            .filter(|hb| !hb.warned)
            .map(|hb| hb.deadline)
            .min()
            .unwrap_or_else(|| now + Duration::from_millis(250));

        match receiver.recv_sync_timeout(next_deadline) {
            Ok(Message::Register(hb)) => {
                heartbeats.insert(hb.id, hb);
            }
            Ok(Message::Complete(id)) => {
                if let Some(hb) = heartbeats.remove(&id) {
                    let now = Instant::now();
                    if !hb.warned && now >= hb.deadline {
                        log_heartbeat(hb.callsite, |record| {
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
        }

        let now = Instant::now();
        for heartbeat in heartbeats.values_mut() {
            if !heartbeat.warned && now >= heartbeat.deadline {
                log_heartbeat(heartbeat.callsite, |record| {
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
#[track_caller]
pub fn heartbeat(name: &'static str, duration: Duration) -> HeartbeatGuard {
    HeartbeatGuard::new(name, duration)
}
