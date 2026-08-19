// SPDX-License-Identifier: MIT OR Apache-2.0

// Native only: this case deliberately races installation/configuration with
// std::thread::scope, and std::thread::spawn is unavailable in browser wasm.
// A non-threaded equivalent runs on both targets in logwise_integration_tests.
#![cfg(not(target_arch = "wasm32"))]

use std::sync::atomic::{AtomicUsize, Ordering};

use logwise::{
    Callsite, Class, ContextToken, Detail, Dispatch, EventRef, FieldMetadata, FieldRef,
    InstallError, Interest, Kind, Metadata, Privacy, Severity, ValueRef, install_dispatcher,
};

struct TestDispatcher {
    generation: AtomicUsize,
    interest: AtomicUsize,
    interest_calls: AtomicUsize,
    emitted: AtomicUsize,
}

impl Dispatch for TestDispatcher {
    fn generation(&self) -> usize {
        self.generation.load(Ordering::Acquire)
    }

    fn interest(&self, metadata: &'static Metadata) -> Interest {
        assert_eq!(metadata.event_name, "logwise.test.dispatch");
        self.interest_calls.fetch_add(1, Ordering::Relaxed);
        Interest::from_bits(self.interest.load(Ordering::Acquire))
    }

    fn emit(&self, event: EventRef<'_>) {
        assert_eq!(event.metadata.event_name, "logwise.test.dispatch");
        assert_eq!(event.fields.len(), 1);
        self.emitted.fetch_add(1, Ordering::Relaxed);
    }
}

static DISPATCHER: TestDispatcher = TestDispatcher {
    generation: AtomicUsize::new(0),
    interest: AtomicUsize::new(Interest::CORE_SUPPORT.bits()),
    interest_calls: AtomicUsize::new(0),
    emitted: AtomicUsize::new(0),
};

static FIELD: FieldMetadata = FieldMetadata::new("answer", Privacy::SupportSafe, Detail::Core);

static METADATA: Metadata = Metadata {
    event_name: "logwise.test.dispatch",
    package: "logwise",
    target: "dispatch",
    module: "dispatch",
    domain: None,
    severity: Severity::Info,
    class: Class::Operational,
    kind: Kind::Event,
    location: None,
    fields: &[FIELD],
};

static CALLSITE: Callsite = Callsite::new(&METADATA);

#[test]
fn dispatcher_is_installed_once_and_interest_is_generation_cached() {
    let successes = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| install_dispatcher(&DISPATCHER)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(Result::is_ok)
            .count()
    });
    assert_eq!(successes, 1);
    assert_eq!(
        install_dispatcher(&DISPATCHER),
        Err(InstallError::AlreadyInstalled)
    );

    assert_eq!(CALLSITE.interest(), Interest::CORE_SUPPORT);
    assert_eq!(CALLSITE.interest(), Interest::CORE_SUPPORT);
    assert_eq!(DISPATCHER.interest_calls.load(Ordering::Relaxed), 1);

    let fields = [Some(FieldRef::new(&FIELD, ValueRef::U64(42)))];
    CALLSITE.emit(EventRef::structured(&METADATA, ContextToken::NONE, &fields));
    assert_eq!(DISPATCHER.emitted.load(Ordering::Relaxed), 1);

    DISPATCHER
        .interest
        .store(Interest::DETAIL_LOCAL.bits(), Ordering::Release);
    assert_eq!(
        CALLSITE.interest(),
        Interest::CORE_SUPPORT,
        "interest changed without a generation change"
    );

    DISPATCHER.generation.fetch_add(1, Ordering::AcqRel);
    assert_eq!(CALLSITE.interest(), Interest::DETAIL_LOCAL);
    assert_eq!(DISPATCHER.interest_calls.load(Ordering::Relaxed), 2);

    std::thread::scope(|scope| {
        scope.spawn(|| {
            for generation in 2..1_000 {
                let interest = if generation % 2 == 0 {
                    Interest::CORE_LOCAL
                } else {
                    Interest::DETAIL_SUPPORT
                };
                DISPATCHER
                    .interest
                    .store(interest.bits(), Ordering::Release);
                DISPATCHER.generation.store(generation, Ordering::Release);
            }
        });

        for _ in 0..8 {
            scope.spawn(|| {
                for _ in 0..2_000 {
                    let interest = CALLSITE.interest();
                    assert!(
                        interest == Interest::DETAIL_LOCAL
                            || interest == Interest::CORE_LOCAL
                            || interest == Interest::DETAIL_SUPPORT
                    );
                }
            });
        }
    });

    DISPATCHER
        .interest
        .store(Interest::CORE_SECRET.bits(), Ordering::Release);
    DISPATCHER.generation.store(1_000, Ordering::Release);
    assert_eq!(CALLSITE.interest(), Interest::CORE_SECRET);
}
