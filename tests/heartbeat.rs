logwise::declare_logging_domain!();

#[cfg(test)]
mod tests {
    use logwise::{heartbeat, InMemoryLogger};
    use logwise::global_logger::{global_loggers, set_global_loggers};
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;
    use test_executors::async_test;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen::JsValue;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_futures::JsFuture;
    #[cfg(target_arch = "wasm32")]
    use web_time::Instant;
    #[cfg(not(target_arch = "wasm32"))]
    use std::{thread};

    static TEST_LOGGER_GUARD: Mutex<()> = Mutex::new(());
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg(target_arch = "wasm32")]
    async fn yield_once() {
        let _ = JsFuture::from(js_sys::Promise::resolve(&JsValue::NULL)).await;
    }

    #[async_test]
    async fn heartbeat_on_time_emits_no_logs() {
        let _guard = TEST_LOGGER_GUARD.lock().unwrap();
        let logger = Arc::new(InMemoryLogger::new());
        let original = global_loggers();
        set_global_loggers(vec![logger.clone()]);

        {
            let _hb = heartbeat("on_time", Duration::from_millis(50));
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
            {
                let start = Instant::now();
                while start.elapsed() < Duration::from_millis(60) {
                    yield_once().await;
                }
            }
        }

        // Give the watcher thread time to emit its warning.
        #[cfg(not(target_arch = "wasm32"))]
        thread::sleep(Duration::from_millis(20));
        #[cfg(target_arch = "wasm32")]
        {
            yield_once().await;
            yield_once().await;
        }
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
