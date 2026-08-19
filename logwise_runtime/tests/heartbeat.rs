// SPDX-License-Identifier: MIT OR Apache-2.0
logwise_runtime::declare_logging_domain!();

#[cfg(test)]
mod tests {
    use logwise_runtime::global_logger::{global_loggers, set_global_loggers};
    use logwise_runtime::{InMemoryLogger, heartbeat};
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::thread;
    use std::time::Duration;

    static TEST_LOGGER_GUARD: Mutex<()> = Mutex::new(());

    // `thread::sleep` is `Atomics.wait`, which traps on the browser main thread,
    // so these run in a worker there — the same shape as `tests/perfwarn_if.rs`.
    #[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test(worker))]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn heartbeat_on_time_emits_no_logs() {
        let _guard = TEST_LOGGER_GUARD.lock().unwrap();
        let logger = Arc::new(InMemoryLogger::new());
        let original = global_loggers();
        set_global_loggers(vec![logger.clone()]);

        {
            let _hb = heartbeat("on_time", Duration::from_millis(200));
        }

        // Ensure the watcher thread had a chance to process completions.
        thread::sleep(Duration::from_millis(20));
        let logs = logger.drain_logs();
        assert!(
            logs.is_empty(),
            "expected no heartbeat logs when dropped on time, got: {logs}"
        );

        set_global_loggers(original);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test(worker))]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn heartbeat_logs_missed_deadline_and_late_drop() {
        let _guard = TEST_LOGGER_GUARD.lock().unwrap();
        let logger = Arc::new(InMemoryLogger::new());
        let original = global_loggers();
        set_global_loggers(vec![logger.clone()]);

        {
            let _hb = heartbeat("frame", Duration::from_millis(5));
            thread::sleep(Duration::from_millis(60));
        }

        // Give the watcher thread time to emit its warning.
        thread::sleep(Duration::from_millis(50));
        let logs = logger.drain_logs();
        assert!(
            logs.contains("missed deadline"),
            "expected missed-deadline warning, got: {logs}"
        );
        assert!(
            logs.contains("dropped after deadline"),
            "expected late-drop warning, got: {logs}"
        );

        set_global_loggers(original);
    }
}
