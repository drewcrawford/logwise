// SPDX-License-Identifier: MIT OR Apache-2.0
logwise::declare_logging_domain!();

#[cfg(test)]
mod tests {
    use logwise::InMemoryLogger;
    use logwise::global_logger::set_global_loggers;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn test_escapes_turbofish_and_call_site_bindings() {
        logwise::context::Context::reset("test_macro_hygiene".to_string());
        let logger = Arc::new(InMemoryLogger::new());
        set_global_loggers(vec![logger.clone()]);

        // The braces of a `\u{...}` escape belong to the escape, not to a placeholder.
        logwise::info_sync!("emoji \u{1F600} {key}", key = 42);

        // A comma inside a turbofish separates generic arguments, not macro arguments.
        logwise::info_sync!("len {len}", len = HashMap::<u8, u8>::new().len());

        // Bindings the expansion introduces must not capture identically named
        // bindings at the call site.
        let record = 1;
        let formatter = 2;
        let id = 3;
        logwise::info_sync!("hygiene {a}{b}{c}", a = record, b = formatter, c = id);

        let logs = logger.drain_logs();
        assert!(
            logs.contains("emoji \u{1F600} 42"),
            "unicode escape was parsed as a placeholder: {logs}"
        );
        assert!(
            logs.contains("len 0"),
            "turbofish argument was truncated at the generic comma: {logs}"
        );
        assert!(
            logs.contains("hygiene 123"),
            "call-site bindings were shadowed by the expansion: {logs}"
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn test_perfwarn_block_sees_call_site_bindings() {
        // `interval` and `result` used to name the expansion's own bindings, so a
        // block referring to call-site bindings of those names did not compile.
        let interval = 10;
        let result = 32;
        let mut runs = 0;
        let value = logwise::perfwarn!("perfwarn hygiene", {
            runs += 1;
            interval + result
        });
        assert_eq!(value, 42);
        assert_eq!(runs, 1);
    }
}
