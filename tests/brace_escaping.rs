// SPDX-License-Identifier: MIT OR Apache-2.0
logwise::declare_logging_domain!();

#[cfg(test)]
mod tests {
    use logwise::InMemoryLogger;
    use logwise::global_logger::set_global_loggers;
    use std::sync::Arc;

    #[test]
    fn test_escaped_braces_render_as_single_braces() {
        logwise::context::Context::reset("test_escaped_braces".to_string());
        let logger = Arc::new(InMemoryLogger::new());
        set_global_loggers(vec![logger.clone()]);

        logwise::info_sync!("escaped {{key}} and closing }} plus {key}", key = 42);

        let logs = logger.drain_logs();
        assert!(
            logs.contains("escaped {key} and closing } plus 42"),
            "unexpected logs: {logs}"
        );
    }
}
