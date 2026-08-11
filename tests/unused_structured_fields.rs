// SPDX-License-Identifier: MIT OR Apache-2.0

use logwise::global_logger::{global_loggers, set_global_loggers};
use logwise::{InMemoryLogger, Logger};
use std::sync::Arc;

logwise::declare_logging_domain!();

struct RestoreLoggers(Vec<Arc<dyn Logger>>);

impl Drop for RestoreLoggers {
    fn drop(&mut self) {
        set_global_loggers(std::mem::take(&mut self.0));
    }
}

#[test]
fn test_structured_fields_not_named_in_the_message_are_preserved() {
    let _restore = RestoreLoggers(global_loggers());
    let logger = Arc::new(InMemoryLogger::new());
    set_global_loggers(vec![logger.clone()]);

    logwise::warn_sync!("request failed", status = 503u16, retryable = true);

    let logs = logger.drain_logs();
    assert!(
        logs.contains("status") && logs.contains("503"),
        "the status field was silently discarded: {logs}"
    );
    assert!(
        logs.contains("retryable") && logs.contains("true"),
        "the retryable field was silently discarded: {logs}"
    );
}
