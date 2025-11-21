logwise::declare_logging_domain!();

#[cfg(test)]
mod tests {
    use logwise::{Level, log_enabled};

    #[test]
    fn test_log_enabled() {
        // In test environment (debug build), Info should be enabled
        assert!(log_enabled!(Level::Info));

        // Trace depends on context, initially false
        assert!(!log_enabled!(Level::Trace));

        // Enable trace
        logwise::context::Context::begin_trace();
        assert!(log_enabled!(Level::Trace));

        // Error is always enabled
        assert!(log_enabled!(Level::Error));
    }

    #[test]
    fn test_level_log() {
        // Just verify it compiles and runs without panic
        Level::Info.log("test message");
    }
}
