// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cross-package acceptance fixtures live here rather than in the
//! zero-dependency facade's test graph.

#[cfg(test)]
mod tests {
    use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use logwise::{
        Callsite, Class, ContextToken, Dispatch, EventRef, Interest, Kind, Metadata, Severity,
        install_dispatcher,
    };

    struct TestDispatcher {
        generation: AtomicUsize,
        interest: AtomicUsize,
        emitted: AtomicUsize,
        materialized: AtomicUsize,
        last_kind: AtomicUsize,
        last_class: AtomicUsize,
        last_severity: AtomicUsize,
    }

    impl Dispatch for TestDispatcher {
        fn generation(&self) -> usize {
            self.generation.load(Ordering::Acquire)
        }

        fn interest(&self, _metadata: &'static Metadata) -> Interest {
            Interest::from_bits(self.interest.load(Ordering::Acquire))
        }

        fn emit(&self, event: EventRef<'_>) {
            assert_eq!(event.metadata.package, "logwise_integration_tests");
            assert_eq!(event.metadata.target, "logwise_integration_tests");
            assert!(event.metadata.module.ends_with("tests"));
            if event.metadata.event_name != "logwise.integration.dispatch" {
                assert!(event.metadata.location.is_some());
                assert_eq!(event.context, ContextToken::from_parts(77, 0));
            }
            if event.metadata.kind == Kind::AdHocText {
                assert_eq!(
                    event.message.expect("ad-hoc message").to_string(),
                    "escaped {brace} 7 state=Ready display=display named=9"
                );
            }
            if event.metadata.event_name == "logwise.integration.selective" {
                assert_eq!(event.metadata.fields.len(), 4);
                assert_eq!(event.metadata.fields[0].name, "task_id");
                assert_eq!(
                    event.metadata.fields[0].privacy,
                    logwise::Privacy::SupportSafe
                );
                assert_eq!(event.metadata.fields[0].detail, logwise::Detail::Core);
                assert_eq!(
                    event.metadata.fields[1].privacy,
                    logwise::Privacy::LocalOnly
                );
                assert_eq!(event.metadata.fields[2].privacy, logwise::Privacy::Secret);
                assert_eq!(event.metadata.fields[3].detail, logwise::Detail::Detail);
            }
            if event.metadata.event_name == "logwise.integration.domain" {
                assert_eq!(
                    event.metadata.domain.expect("domain override").name,
                    "some_executor.scheduler"
                );
            }
            self.emitted.fetch_add(1, Ordering::Relaxed);
            self.materialized
                .store(event.fields.iter().flatten().count(), Ordering::Relaxed);
            self.last_kind
                .store(event.metadata.kind as usize, Ordering::Relaxed);
            self.last_class
                .store(event.metadata.class as usize, Ordering::Relaxed);
            self.last_severity
                .store(event.metadata.severity as usize, Ordering::Relaxed);
        }

        fn capture_context(&self) -> ContextToken {
            ContextToken::from_parts(77, 0)
        }
    }

    static DISPATCHER: TestDispatcher = TestDispatcher {
        generation: AtomicUsize::new(0),
        interest: AtomicUsize::new(Interest::CORE_LOCAL.bits()),
        emitted: AtomicUsize::new(0),
        materialized: AtomicUsize::new(0),
        last_kind: AtomicUsize::new(0),
        last_class: AtomicUsize::new(0),
        last_severity: AtomicUsize::new(0),
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

        let no_runtime_macro_evaluated = AtomicBool::new(false);
        logwise::log!("value={}", {
            no_runtime_macro_evaluated.store(true, Ordering::Relaxed);
            1
        });
        logwise::event!(
            "logwise.integration.no_runtime",
            value = support({
                no_runtime_macro_evaluated.store(true, Ordering::Relaxed);
                1_u64
            }),
        );
        assert!(!no_runtime_macro_evaluated.load(Ordering::Relaxed));

        install_dispatcher(&DISPATCHER).expect("install test dispatcher");
        assert_eq!(CALLSITE.interest(), Interest::CORE_LOCAL);
        CALLSITE.emit(EventRef::structured(&METADATA, ContextToken::NONE, &[]));
        assert_eq!(DISPATCHER.emitted.load(Ordering::Relaxed), 1);

        #[derive(Debug)]
        enum State {
            Ready,
        }

        struct Shown;
        impl core::fmt::Display for Shown {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("display")
            }
        }

        let state = State::Ready;
        let named = 9;
        logwise::log!(
            "escaped {{brace}} {} state={state:?} display={} named={named}",
            7,
            Shown,
        );
        assert_eq!(
            DISPATCHER.last_kind.load(Ordering::Relaxed),
            Kind::AdHocText as usize
        );

        let mut evaluated = 0;
        DISPATCHER
            .interest
            .store(Interest::CORE_SUPPORT.bits(), Ordering::Release);
        DISPATCHER.generation.fetch_add(1, Ordering::AcqRel);
        let opaque_text_evaluated = AtomicBool::new(false);
        logwise::log!("private={}", {
            opaque_text_evaluated.store(true, Ordering::Relaxed);
            "private"
        });
        assert!(!opaque_text_evaluated.load(Ordering::Relaxed));
        logwise::event!(
            "logwise.integration.selective",
            task_id = support({ evaluated += 1; 42_u64 }),
            task_name = local({ evaluated += 10; "private" }),
            credential = secret({ evaluated += 1_000; "secret" }),
            detail route = local({ evaluated += 100; "expensive" }),
        );
        assert_eq!(evaluated, 1);
        assert_eq!(DISPATCHER.materialized.load(Ordering::Relaxed), 1);

        DISPATCHER
            .interest
            .store(Interest::DETAIL_LOCAL.bits(), Ordering::Release);
        DISPATCHER.generation.fetch_add(1, Ordering::AcqRel);
        logwise::event!(
            class: forensic,
            severity: warn,
            name: "logwise.integration.explicit",
            task_id = support({ evaluated += 1; 42_u64 }),
            detail route = local({ evaluated += 100; "expensive" }),
        );
        assert_eq!(evaluated, 101);
        assert_eq!(DISPATCHER.materialized.load(Ordering::Relaxed), 1);
        assert_eq!(
            DISPATCHER.last_class.load(Ordering::Relaxed),
            Class::Forensic as usize
        );
        assert_eq!(
            DISPATCHER.last_severity.load(Ordering::Relaxed),
            Severity::Warn as usize
        );

        drop(logwise::span!("logwise.integration.span"));
        assert_eq!(
            DISPATCHER.last_kind.load(Ordering::Relaxed),
            Kind::Span as usize
        );
        logwise::counter!("logwise.integration.counter");
        assert_eq!(
            DISPATCHER.last_kind.load(Ordering::Relaxed),
            Kind::Counter as usize
        );
        logwise::measurement!("logwise.integration.measurement");
        assert_eq!(
            DISPATCHER.last_kind.load(Ordering::Relaxed),
            Kind::Measurement as usize
        );

        let __logwise_interest = "call-site binding";
        let __logwise_metadata = "call-site binding";
        logwise::event!("logwise.integration.hygiene", value = 1_u8);
        assert_eq!(__logwise_interest, "call-site binding");
        assert_eq!(__logwise_metadata, "call-site binding");

        let excluded_evaluated = AtomicBool::new(false);
        logwise::event!(
            #[cfg(any())]
            "LOGWISE_STATICALLY_EXCLUDED_SENTINEL",
            value = {
                excluded_evaluated.store(true, Ordering::Relaxed);
                1_u8
            },
        );
        assert!(!excluded_evaluated.load(Ordering::Relaxed));

        const SCHEDULER: logwise::Domain = logwise::domain!("some_executor.scheduler");
        assert_eq!(SCHEDULER.name, "some_executor.scheduler");
        logwise::event!(
            domain: SCHEDULER,
            name: "logwise.integration.domain",
            task_id = support(42_u64),
        );
    }
}
