// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests that `debuginternal_sync!` works correctly in integration tests,
//! which are considered the "current crate" and should have debuginternal enabled.

logwise::declare_logging_domain!();

use logwise::context::Context;
use logwise::{InMemoryLogger, add_global_logger};
use std::sync::Arc;

#[test]
fn test_debuginternal_enabled_in_integration_test() {
    Context::reset("test_debuginternal".to_string());

    let logger = Arc::new(InMemoryLogger::new());
    add_global_logger(logger.clone());

    logwise::debuginternal_sync!("test message");

    let logs = logger.drain_logs();
    assert!(
        !logs.is_empty(),
        "debuginternal should be enabled in integration tests"
    );
    assert!(logs.contains("test message"));
}
