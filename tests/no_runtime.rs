// SPDX-License-Identifier: MIT OR Apache-2.0

// Native only: this test observes the process System allocator around the
// no-runtime fast path. Browser wasm uses a different allocator/test harness;
// its no-runtime evaluation check lives in logwise_integration_tests.
#![cfg(not(target_arch = "wasm32"))]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use logwise::{Callsite, Class, Kind, Metadata, Severity};

struct CountingAllocator;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

// SAFETY: this delegates every operation to System with the original pointer
// and layout, adding only an atomic observation before successful allocation.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the GlobalAlloc layout contract unchanged.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the pointer/layout pair came from System through alloc above.
        unsafe { System.dealloc(pointer, layout) };
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

static METADATA: Metadata = Metadata {
    event_name: "logwise.test.no_runtime",
    package: "logwise",
    target: "no_runtime",
    module: "no_runtime",
    domain: None,
    severity: Severity::Debug,
    class: Class::Diagnostic,
    kind: Kind::Event,
    location: None,
    fields: &[],
};

static CALLSITE: Callsite = Callsite::new(&METADATA);

#[test]
fn no_runtime_rejects_before_evaluation_without_allocating() {
    assert_eq!(core::mem::size_of::<logwise::ContextToken>(), 16);
    assert_eq!(core::mem::size_of::<logwise::SpanToken>(), 16);
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    let mut evaluations = 0;

    for _ in 0..1_000 {
        let interest = CALLSITE.interest();
        if interest.any() {
            evaluations += 1;
        }
        logwise::log!("value={}", {
            evaluations += 1;
            1
        });
        logwise::event!(
            "logwise.test.no_runtime.macro",
            value = support({
                evaluations += 1;
                1_u64
            }),
        );
    }

    let after = ALLOCATIONS.load(Ordering::Relaxed);
    assert_eq!(evaluations, 0);
    assert_eq!(after, before, "no-runtime interest checks allocated");

    assert!(logwise::context::capture().is_none());
    let child = logwise::context::child(logwise::ContextToken::NONE, "no-runtime");
    assert!(child.is_none());
    let _entered = logwise::context::enter(child);
    assert!(logwise::context::capture().is_none());
}
