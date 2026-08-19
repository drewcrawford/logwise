// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(target_arch = "wasm32")]
#[wasm_lite::wasm_lite_test]
fn test_error_record_named_debugme_does_not_panic() {
    let mut record = logwise_runtime::LogRecord::new(logwise_runtime::Level::Error);
    record.log("DEBUGME");

    for logger in logwise_runtime::global_loggers() {
        logger.finish_log_record(record.clone());
    }
}

#[cfg(target_arch = "wasm32")]
wasm_lite::test_main!();

#[cfg(not(target_arch = "wasm32"))]
fn main() {}
