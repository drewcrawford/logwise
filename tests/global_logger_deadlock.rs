// SPDX-License-Identifier: MIT OR Apache-2.0

// Native only: the deadlock is only observable as a hang, so the test re-runs
// itself as a subprocess and lets a watchdog thread `process::exit` out of it.
// The browser has neither `current_exe` nor a subprocess to re-enter, and a
// genuine deadlock there would hang the runner rather than fail a case.
#![cfg(not(target_arch = "wasm32"))]

use logwise::{InMemoryLogger, LogRecord, Logger, global_loggers, set_global_loggers};
use std::future::Future;
use std::pin::Pin;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug)]
struct ReentrantDropLogger;

impl Drop for ReentrantDropLogger {
    fn drop(&mut self) {
        let _ = global_loggers();
    }
}

impl Logger for ReentrantDropLogger {
    fn finish_log_record(&self, _record: LogRecord) {}

    fn finish_log_record_async<'s>(
        &'s self,
        _record: LogRecord,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 's>> {
        Box::pin(async {})
    }

    fn prepare_to_die(&self) {}
}

#[test]
#[ignore = "run in a subprocess by test_replacing_loggers_drops_them_after_unlocking"]
fn helper_reentrant_logger_drop_must_not_deadlock() {
    let watchdog = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(2));
        std::process::exit(86);
    });

    set_global_loggers(vec![Arc::new(ReentrantDropLogger)]);
    set_global_loggers(vec![Arc::new(InMemoryLogger::new())]);

    drop(watchdog);
}

#[test]
fn test_replacing_loggers_drops_them_after_unlocking() {
    let output = Command::new(std::env::current_exe().expect("locate integration test executable"))
        .arg("--ignored")
        .arg("--exact")
        .arg("helper_reentrant_logger_drop_must_not_deadlock")
        .arg("--nocapture")
        .output()
        .expect("run re-entrant logger drop regression in a subprocess");

    assert!(
        output.status.success(),
        "replacing global loggers deadlocked while dropping a logger; child status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
