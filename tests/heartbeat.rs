// SPDX-License-Identifier: MIT OR Apache-2.0
logwise::declare_logging_domain!();

#[cfg(test)]
mod tests {
    use logwise::global_logger::{global_loggers, set_global_loggers};
    use logwise::{InMemoryLogger, heartbeat};
    use std::sync::Arc;
    use std::sync::Mutex;
    #[cfg(not(target_arch = "wasm32"))]
    use std::thread;
    use std::time::Duration;
    use test_executors::async_test;
    #[cfg(target_arch = "wasm32")]

    static TEST_LOGGER_GUARD: Mutex<()> = Mutex::new(());

    /// Let the event loop turn once, so a spawned task gets a chance to run.
    #[cfg(target_arch = "wasm32")]
    async fn yield_once() {
        wasm_lite_std::yield_to_event_loop_async().await;
    }

    #[cfg(target_arch = "wasm32")]
    async fn sleep_ms(ms: i32) {
        wasm_lite_std::sleep_async(std::time::Duration::from_millis(ms as u64)).await;
    }

    #[async_test]
    async fn heartbeat_on_time_emits_no_logs() {
        let _guard = TEST_LOGGER_GUARD.lock().unwrap();
        let logger = Arc::new(InMemoryLogger::new());
        let original = global_loggers();
        set_global_loggers(vec![logger.clone()]);

        {
            let _hb = heartbeat("on_time", Duration::from_millis(200));
        }

        // Ensure background thread had a chance to process completions.
        #[cfg(not(target_arch = "wasm32"))]
        thread::sleep(Duration::from_millis(10));
        #[cfg(target_arch = "wasm32")]
        yield_once().await;
        let logs = logger.drain_logs();
        assert!(
            logs.is_empty(),
            "expected no heartbeat logs when dropped on time, got: {logs}"
        );

        set_global_loggers(original);
    }

    #[async_test]
    async fn heartbeat_logs_missed_deadline_and_late_drop() {
        let _guard = TEST_LOGGER_GUARD.lock().unwrap();
        let logger = Arc::new(InMemoryLogger::new());
        let original = global_loggers();
        set_global_loggers(vec![logger.clone()]);

        {
            let _hb = heartbeat("frame", Duration::from_millis(5));
            #[cfg(not(target_arch = "wasm32"))]
            thread::sleep(Duration::from_millis(60));
            #[cfg(target_arch = "wasm32")]
            sleep_ms(60).await;
        }

        // Give the watcher thread time to emit its warning.
        #[cfg(not(target_arch = "wasm32"))]
        thread::sleep(Duration::from_millis(20));
        #[cfg(target_arch = "wasm32")]
        sleep_ms(50).await;
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
