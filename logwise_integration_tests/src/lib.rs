// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cross-package acceptance fixtures live here rather than in the
//! zero-dependency facade's test graph.

#[cfg(test)]
mod tests {
    use core::sync::atomic::{AtomicUsize, Ordering};

    use logwise::{
        Callsite, Class, ContextToken, Dispatch, EventRef, Interest, Kind, Metadata, Severity,
        install_dispatcher,
    };

    struct TestDispatcher {
        generation: AtomicUsize,
        emitted: AtomicUsize,
    }

    impl Dispatch for TestDispatcher {
        fn generation(&self) -> usize {
            self.generation.load(Ordering::Acquire)
        }

        fn interest(&self, _metadata: &'static Metadata) -> Interest {
            Interest::CORE_LOCAL
        }

        fn emit(&self, _event: EventRef<'_>) {
            self.emitted.fetch_add(1, Ordering::Relaxed);
        }
    }

    static DISPATCHER: TestDispatcher = TestDispatcher {
        generation: AtomicUsize::new(0),
        emitted: AtomicUsize::new(0),
    };

    static METADATA: Metadata = Metadata {
        event_name: "logwise.integration.dispatch",
        package: "logwise_integration_tests",
        target: "logwise_integration_tests",
        module: "tests",
        domain: None,
        severity: Severity::Debug,
        class: Class::Diagnostic,
        kind: Kind::Event,
        location: None,
        fields: &[],
    };

    static CALLSITE: Callsite = Callsite::new(&METADATA);

    #[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn no_runtime_then_synchronous_dispatch() {
        let mut evaluated = false;
        if CALLSITE.interest().any() {
            evaluated = true;
        }
        assert!(!evaluated, "no-runtime path evaluated an enabled branch");

        install_dispatcher(&DISPATCHER).expect("install test dispatcher");
        assert_eq!(CALLSITE.interest(), Interest::CORE_LOCAL);
        CALLSITE.emit(EventRef::structured(&METADATA, ContextToken::NONE, &[]));
        assert_eq!(DISPATCHER.emitted.load(Ordering::Relaxed), 1);
    }
}
