// SPDX-License-Identifier: MIT OR Apache-2.0

use logwise_runtime::global_logger::{global_loggers, set_global_loggers};
use logwise_runtime::{InMemoryLogger, Level, Logger};
use std::sync::Arc;

logwise_runtime::declare_logging_domain!();

struct RestoreLoggers(Vec<Arc<dyn Logger>>);

impl Drop for RestoreLoggers {
    fn drop(&mut self) {
        set_global_loggers(std::mem::take(&mut self.0));
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn test_interval_end_uses_the_context_where_the_interval_started() {
    let origin =
        logwise_runtime::context::Context::new_task(None, "origin".to_string(), Level::Info, false);
    let origin_task = origin.task_id();
    origin.set_current();

    let _restore = RestoreLoggers(global_loggers());
    let logger = Arc::new(InMemoryLogger::new());
    set_global_loggers(vec![logger.clone()]);

    let interval = logwise_runtime::perfwarn_begin!("cross_context_interval");

    let unrelated = logwise_runtime::context::Context::new_task(
        None,
        "unrelated".to_string(),
        Level::Info,
        false,
    );
    unrelated.set_current();
    drop(interval);

    let logs = logger.drain_logs();
    assert!(
        logs.lines().any(|line| {
            line.contains(&format!(" {origin_task} PERFWARN: BEGIN"))
                && line.contains("cross_context_interval")
        }),
        "the BEGIN record should establish the originating task: {logs}"
    );
    assert!(
        logs.lines().any(|line| {
            line.contains(&format!(" {origin_task} PERFWARN: END"))
                && line.contains("cross_context_interval")
        }),
        "the END record was not attributed to the interval's originating task: {logs}"
    );
}
