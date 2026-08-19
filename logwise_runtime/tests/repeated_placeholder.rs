// SPDX-License-Identifier: MIT OR Apache-2.0

//! A `{key}` may appear more than once in a format string. The value expression
//! behind it must still be evaluated exactly once.

use logwise_runtime::global_logger::{global_loggers, set_global_loggers};
use logwise_runtime::{InMemoryLogger, Logger};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

logwise_runtime::declare_logging_domain!();

struct RestoreLoggers(Vec<Arc<dyn Logger>>);

impl Drop for RestoreLoggers {
    fn drop(&mut self) {
        set_global_loggers(std::mem::take(&mut self.0));
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn test_repeated_placeholder_evaluates_its_value_once() {
    let _restore = RestoreLoggers(global_loggers());
    let logger = Arc::new(InMemoryLogger::new());
    set_global_loggers(vec![logger.clone()]);

    let evaluations = AtomicU32::new(0);
    logwise_runtime::warn_sync!(
        "attempt {n}, retrying attempt {n}",
        n = {
            evaluations.fetch_add(1, Ordering::Relaxed);
            7u8
        }
    );

    let logs = logger.drain_logs();
    assert_eq!(
        evaluations.load(Ordering::Relaxed),
        1,
        "the value expression ran once per placeholder occurrence: {logs}"
    );
    assert!(
        logs.contains("attempt 7, retrying attempt 7"),
        "the two occurrences disagreed about the value: {logs}"
    );

    // Splicing the expression once per occurrence would also move `owned` twice
    // and fail to compile, pointing the user at generated code.
    let owned = String::from("payload");
    logwise_runtime::warn_sync!("{body} == {body}", body = owned);

    let logs = logger.drain_logs();
    assert!(
        logs.contains("payload == payload"),
        "a moved value was not logged for both occurrences: {logs}"
    );
}
