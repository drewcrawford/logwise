// SPDX-License-Identifier: MIT OR Apache-2.0

//! Entered tracing spans do not have to be exited in the order they were
//! entered, and the bridge has to survive that.
//!
//! `tracing::span::Entered` guards are ordinary values, so `drop(outer)` before
//! `drop(inner)` is legal and produces `exit(outer)` before `exit(inner)`. The
//! bridge holds one logwise `ContextGuard` per entered span, and each guard
//! restores the token that was current when it was created, so the stack has to
//! be unwound from the top down rather than matched positionally.
//!
//! Native-only: `tracing-subscriber`'s registry is the subject here and this
//! package has no browser test harness, matching the crate's other test.

use std::cell::Cell;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use logwise::{ContextToken, Dispatch, EventRef, Interest, Metadata};
use logwise_compat_tracing::LogwiseLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::Registry;

thread_local! {
    static CURRENT: Cell<ContextToken> = const { Cell::new(ContextToken::NONE) };
}

struct Capture {
    next_context: AtomicU64,
    names: Mutex<Vec<(ContextToken, &'static str)>>,
}

impl Dispatch for Capture {
    fn generation(&self) -> usize {
        0
    }

    fn interest(&self, _metadata: &'static Metadata) -> Interest {
        Interest::CORE_LOCAL
    }

    fn emit(&self, _event: EventRef<'_>) {}

    fn capture_context(&self) -> ContextToken {
        CURRENT.with(Cell::get)
    }

    fn create_context(&self, _parent: ContextToken, name: &'static str) -> ContextToken {
        let token =
            ContextToken::from_parts(self.next_context.fetch_add(1, Ordering::Relaxed) + 1, 0);
        self.names.lock().unwrap().push((token, name));
        token
    }

    fn enter_context(&self, context: ContextToken) -> ContextToken {
        CURRENT.with(|current| current.replace(context))
    }

    fn exit_context(&self, previous: ContextToken) {
        CURRENT.with(|current| current.set(previous));
    }
}

static CAPTURE: Capture = Capture {
    next_context: AtomicU64::new(0),
    names: Mutex::new(Vec::new()),
};

#[test]
fn exiting_spans_out_of_order_still_restores_the_entry_context() {
    logwise::install_dispatcher(&CAPTURE).expect("install capture dispatcher");

    let subscriber = Registry::default().with(LogwiseLayer::new());
    tracing::subscriber::with_default(subscriber, || {
        assert_eq!(logwise::context::capture(), ContextToken::NONE);

        let outer = tracing::info_span!("outer");
        let inner = tracing::info_span!("inner");

        let outer_entered = outer.enter();
        let outer_context = logwise::context::capture();
        assert!(
            !outer_context.is_none(),
            "entering a span enters its context"
        );

        let inner_entered = inner.enter();
        let inner_context = logwise::context::capture();
        assert_ne!(inner_context, outer_context);

        // Legal, and the reason the bridge cannot assume its stack is matched
        // top-of-stack-first.
        drop(outer_entered);
        assert_eq!(
            logwise::context::capture(),
            inner_context,
            "the inner span is still entered, so its context stays current"
        );

        drop(inner_entered);

        assert_eq!(
            logwise::context::capture(),
            ContextToken::NONE,
            "both spans were exited, so no logwise context may still be entered"
        );
    });

    assert_eq!(logwise::context::capture(), ContextToken::NONE);
}
