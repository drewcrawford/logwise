// SPDX-License-Identifier: MIT OR Apache-2.0

use logwise_runtime::global_logger::{global_loggers, set_global_loggers};
use logwise_runtime::privacy::LogIt;
use logwise_runtime::{LogRecord, Logger};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;

logwise_runtime::declare_logging_domain!();

struct RestoreLoggers(Vec<Arc<dyn Logger>>);

impl Drop for RestoreLoggers {
    fn drop(&mut self) {
        set_global_loggers(std::mem::take(&mut self.0));
    }
}

#[derive(Debug, Default)]
struct RemoteLogger {
    records: Mutex<Vec<String>>,
}

impl RemoteLogger {
    fn drain_logs(&self) -> String {
        let mut records = self.records.lock().unwrap();
        let logs = records.join("\n");
        records.clear();
        logs
    }
}

impl Logger for RemoteLogger {
    fn finish_log_record(&self, record: LogRecord) {
        self.records.lock().unwrap().push(record.to_string());
    }

    fn finish_log_record_async<'s>(
        &'s self,
        record: LogRecord,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 's>> {
        Box::pin(async move { self.finish_log_record(record) })
    }

    fn prepare_to_die(&self) {}
}

#[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn test_private_values_are_redacted_before_reaching_remote_loggers() {
    let _restore = RestoreLoggers(global_loggers());
    let remote_sink = Arc::new(RemoteLogger::default());
    set_global_loggers(vec![remote_sink.clone()]);

    logwise_runtime::warn_sync!(
        "authentication failed for {token}",
        token = LogIt("secret-token")
    );

    let transmitted = remote_sink.drain_logs();
    assert!(
        !transmitted.contains("secret-token"),
        "a logger that may transmit records remotely received private data: {transmitted}"
    );
    assert!(
        transmitted.contains("<LogIt>"),
        "the private value should have been replaced by its redacted representation: {transmitted}"
    );
}
