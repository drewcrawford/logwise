// SPDX-License-Identifier: MIT OR Apache-2.0

use logwise_runtime::global_logger::{global_loggers, set_global_loggers};
use logwise_runtime::{InMemoryLogger, Level, Logger};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

logwise_runtime::declare_logging_domain!();

struct RestoreLoggers(Vec<Arc<dyn Logger>>);

impl Drop for RestoreLoggers {
    fn drop(&mut self) {
        set_global_loggers(std::mem::take(&mut self.0));
    }
}

// `thread::sleep` is `Atomics.wait`, which traps on the browser main thread, so
// this runs in a worker there — the same shape as `tests/heartbeat.rs`.
#[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test(worker))]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn test_missed_heartbeat_uses_the_creating_context() {
    let origin = logwise_runtime::context::Context::new_task(
        None,
        "heartbeat_origin".to_string(),
        Level::Info,
        false,
    );
    let origin_task = origin.task_id();
    origin.set_current();

    let _restore = RestoreLoggers(global_loggers());
    let logger = Arc::new(InMemoryLogger::new());
    set_global_loggers(vec![logger.clone()]);

    let heartbeat = logwise_runtime::heartbeat("contextual_heartbeat", Duration::from_millis(10));
    thread::sleep(Duration::from_millis(100));
    drop(heartbeat);
    thread::sleep(Duration::from_millis(30));

    let logs = logger.drain_logs();
    let missed_deadline = logs
        .lines()
        .find(|line| line.contains("contextual_heartbeat") && line.contains("missed deadline"))
        .unwrap_or_else(|| panic!("the watcher did not emit a missed-deadline record: {logs}"));
    assert!(
        missed_deadline.contains(&format!(" {origin_task} PERFWARN: HEARTBEAT")),
        "the watcher attributed the heartbeat to its own thread instead of the creator: {missed_deadline}"
    );
}
