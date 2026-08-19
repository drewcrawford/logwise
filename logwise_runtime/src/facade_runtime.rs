// SPDX-License-Identifier: MIT OR Apache-2.0

//! Runtime-owned context storage and monotonic span timing for the facade.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use logwise::{
    ContextToken, Dispatch, EventRef, InstallError, Interest, Metadata, SpanRef, SpanTiming,
    SpanToken, install_dispatcher,
};

use crate::spinlock::Spinlock;
use crate::sys::{Duration, Instant};

std::thread_local! {
    static CURRENT_CONTEXT: Cell<ContextToken> = const { Cell::new(ContextToken::NONE) };
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextSnapshot {
    pub token: ContextToken,
    pub name: &'static str,
    pub parent: Option<ContextToken>,
    pub links: Vec<ContextToken>,
}

#[derive(Clone, Debug)]
pub struct CompletedSpan {
    pub token: SpanToken,
    pub event_name: &'static str,
    pub context: ContextToken,
    pub timing: SpanTiming,
    pub elapsed: Duration,
    pub warning_threshold: Option<Duration>,
    pub threshold_exceeded: bool,
}

#[derive(Debug)]
struct ActiveSpan {
    event_name: &'static str,
    context: ContextToken,
    timing: SpanTiming,
    started: Instant,
    warning_threshold: Option<Duration>,
}

#[derive(Debug)]
struct Activation {
    root: ContextToken,
    interest: Interest,
    expires: Instant,
}

#[derive(Debug, Default)]
struct State {
    contexts: HashMap<u64, ContextSnapshot>,
    active_spans: HashMap<u64, ActiveSpan>,
    completed_spans: Vec<CompletedSpan>,
    activations: Vec<Activation>,
}

/// The mutable runtime installed behind logwise's stable facade dispatcher.
pub struct Runtime {
    generation: AtomicUsize,
    baseline_interest: AtomicUsize,
    next_context: AtomicU64,
    next_span: AtomicU64,
    state: Spinlock<State>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            generation: AtomicUsize::new(0),
            baseline_interest: AtomicUsize::new(Interest::NONE.bits()),
            next_context: AtomicU64::new(1),
            next_span: AtomicU64::new(1),
            state: Spinlock::new(State::default()),
        }
    }

    /// Changes the field groups recorded without context targeting.
    pub fn set_interest(&self, interest: Interest) {
        self.baseline_interest
            .store(interest.without_contextual().bits(), Ordering::Release);
        self.advance_generation();
    }

    /// Enables additional detail for a context and its descendants until TTL.
    pub fn activate_context(&self, root: ContextToken, interest: Interest, ttl: Duration) {
        let expires = Instant::now() + ttl;
        self.state.with_mut(|state| {
            state.activations.push(Activation {
                root,
                interest: interest.without_contextual(),
                expires,
            });
        });
        self.advance_generation();
    }

    pub fn context_is_active(&self, context: ContextToken) -> bool {
        self.contextual_activation(context).any()
    }

    pub fn context(&self, token: ContextToken) -> Option<ContextSnapshot> {
        let id = token.into_parts().0;
        self.state.with(|state| state.contexts.get(&id).cloned())
    }

    pub fn take_completed_spans(&self) -> Vec<CompletedSpan> {
        self.state
            .with_mut(|state| std::mem::take(&mut state.completed_spans))
    }

    fn advance_generation(&self) {
        let previous = self.generation.fetch_add(1, Ordering::AcqRel);
        assert_ne!(previous, usize::MAX - 1, "logwise generation exhausted");
    }

    fn contextual_activation(&self, context: ContextToken) -> Interest {
        let now = Instant::now();
        let (interest, removed_expired) = self.state.with_mut(|state| {
            let before = state.activations.len();
            state
                .activations
                .retain(|activation| activation.expires > now);
            let mut interest = Interest::NONE;
            for activation in &state.activations {
                if is_descendant(&state.contexts, context, activation.root) {
                    interest = interest.union(activation.interest);
                }
            }
            (interest, before != state.activations.len())
        });
        if removed_expired {
            self.advance_generation();
        }
        interest
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

fn is_descendant(
    contexts: &HashMap<u64, ContextSnapshot>,
    mut candidate: ContextToken,
    ancestor: ContextToken,
) -> bool {
    while !candidate.is_none() {
        if candidate == ancestor {
            return true;
        }
        let id = candidate.into_parts().0;
        let Some(snapshot) = contexts.get(&id) else {
            return false;
        };
        let Some(parent) = snapshot.parent else {
            return false;
        };
        candidate = parent;
    }
    false
}

impl Dispatch for Runtime {
    fn generation(&self) -> usize {
        self.generation.load(Ordering::Acquire)
    }

    fn interest(&self, _metadata: &'static Metadata) -> Interest {
        let baseline = Interest::from_bits(self.baseline_interest.load(Ordering::Acquire));
        if self.state.with(|state| state.activations.is_empty()) {
            baseline
        } else {
            baseline.union(Interest::CONTEXTUAL)
        }
    }

    fn contextual_interest(&self, _metadata: &'static Metadata, context: ContextToken) -> Interest {
        Interest::from_bits(self.baseline_interest.load(Ordering::Acquire))
            .union(self.contextual_activation(context))
    }

    fn emit(&self, _event: EventRef<'_>) {}

    fn capture_context(&self) -> ContextToken {
        CURRENT_CONTEXT.get()
    }

    fn create_context(&self, parent: ContextToken, name: &'static str) -> ContextToken {
        let id = self.next_context.fetch_add(1, Ordering::Relaxed);
        assert_ne!(id, u64::MAX, "logwise context IDs exhausted");
        let token = ContextToken::from_parts(id, 0);
        self.state.with_mut(|state| {
            state.contexts.insert(
                id,
                ContextSnapshot {
                    token,
                    name,
                    parent: (!parent.is_none()).then_some(parent),
                    links: Vec::new(),
                },
            );
        });
        token
    }

    fn link_context(&self, context: ContextToken, related: ContextToken) {
        let id = context.into_parts().0;
        self.state.with_mut(|state| {
            if let Some(snapshot) = state.contexts.get_mut(&id)
                && !snapshot.links.contains(&related)
            {
                snapshot.links.push(related);
            }
        });
    }

    fn enter_context(&self, context: ContextToken) -> ContextToken {
        CURRENT_CONTEXT.replace(context)
    }

    fn exit_context(&self, previous: ContextToken) {
        CURRENT_CONTEXT.set(previous);
    }

    fn start_span(&self, span: SpanRef<'_>) -> SpanToken {
        let id = self.next_span.fetch_add(1, Ordering::Relaxed);
        assert_ne!(id, u64::MAX, "logwise span IDs exhausted");
        let token = SpanToken::from_parts(id, 0);
        let active = ActiveSpan {
            event_name: span.event.metadata.event_name,
            context: span.event.context,
            timing: span.timing,
            started: Instant::now(),
            warning_threshold: span.warning_threshold,
        };
        self.state.with_mut(|state| {
            state.active_spans.insert(id, active);
        });
        token
    }

    fn end_span(&self, span: SpanToken, captured_context: ContextToken) {
        let id = span.into_parts().0;
        let Some(active) = self.state.with_mut(|state| state.active_spans.remove(&id)) else {
            return;
        };
        debug_assert_eq!(active.context, captured_context);
        let elapsed = active.started.elapsed();
        let threshold_exceeded = active
            .warning_threshold
            .is_some_and(|threshold| elapsed >= threshold);
        self.state.with_mut(|state| {
            state.completed_spans.push(CompletedSpan {
                token: span,
                event_name: active.event_name,
                context: captured_context,
                timing: active.timing,
                elapsed,
                warning_threshold: active.warning_threshold,
                threshold_exceeded,
            });
        });
    }
}

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Installs the standard runtime dispatcher once and returns its mutable core.
pub fn init() -> Result<&'static Runtime, InstallError> {
    let runtime = RUNTIME.get_or_init(Runtime::new);
    install_dispatcher(runtime)?;
    Ok(runtime)
}

#[cfg(test)]
mod tests {
    use super::*;

    static METADATA: Metadata = Metadata {
        event_name: "logwise_runtime.test.context",
        package: "logwise_runtime",
        target: "logwise_runtime",
        module: "facade_runtime::tests",
        domain: None,
        severity: logwise::Severity::Debug,
        class: logwise::Class::Diagnostic,
        kind: logwise::Kind::Event,
        location: None,
        fields: &[],
    };

    #[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test(worker))]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn context_migration_links_activation_and_span_timing() {
        let runtime = init().expect("install facade runtime");
        runtime.set_interest(Interest::CORE_LOCAL);

        assert!(logwise::context::capture().is_none());
        let root = logwise::context::child(ContextToken::NONE, "root");
        let related = logwise::context::child(ContextToken::NONE, "related");
        let child = logwise::context::child(root, "child");
        logwise::context::link(child, related);

        let snapshot = runtime.context(child).expect("child snapshot");
        assert_eq!(snapshot.parent, Some(root));
        assert_eq!(snapshot.links, vec![related]);

        {
            let _root = logwise::context::enter(root);
            assert_eq!(logwise::context::capture(), root);
            {
                let _child = logwise::context::enter(child);
                assert_eq!(logwise::context::capture(), child);
            }
            assert_eq!(logwise::context::capture(), root);

            let migrated = wasm_lite_std::spawn(move || {
                assert!(logwise::context::capture().is_none());
                let _entered = logwise::context::enter(child);
                assert_eq!(logwise::context::capture(), child);
            });
            migrated.join().expect("migrated task thread");

            let warning = logwise::perfwarn!(
                threshold: Duration::ZERO,
                name: "logwise_runtime.test.wall"
            );
            {
                let _other = logwise::context::enter(related);
                drop(warning);
            }
            let _active = logwise::active_span!("logwise_runtime.test.active");
            let _wake = logwise::wake_latency_span!("logwise_runtime.test.wake");
        }
        assert!(logwise::context::capture().is_none());

        let completed = runtime.take_completed_spans();
        assert_eq!(completed.len(), 3);
        let wall = completed
            .iter()
            .find(|span| span.timing == SpanTiming::WallTime)
            .expect("wall span");
        assert_eq!(wall.context, root);
        assert!(wall.threshold_exceeded);
        assert!(
            completed
                .iter()
                .any(|span| span.timing == SpanTiming::ActiveTime)
        );
        assert!(
            completed
                .iter()
                .any(|span| span.timing == SpanTiming::WakeLatency)
        );

        runtime.set_interest(Interest::NONE);
        runtime.activate_context(root, Interest::DETAIL_LOCAL, Duration::from_secs(60));
        assert!(runtime.interest(&METADATA).is_contextual());
        assert_eq!(
            runtime.contextual_interest(&METADATA, child),
            Interest::DETAIL_LOCAL
        );
        assert_eq!(
            runtime.contextual_interest(&METADATA, related),
            Interest::NONE
        );

        let expired = logwise::context::child(ContextToken::NONE, "expired");
        runtime.activate_context(expired, Interest::CORE_LOCAL, Duration::ZERO);
        assert!(!runtime.context_is_active(expired));

        let threshold_evaluated = std::sync::atomic::AtomicBool::new(false);
        let disabled = logwise::perfwarn!(
            threshold: {
                threshold_evaluated.store(true, Ordering::Relaxed);
                Duration::ZERO
            },
            name: "logwise_runtime.test.disabled"
        );
        drop(disabled);
        assert!(!threshold_evaluated.load(Ordering::Relaxed));
        assert!(runtime.take_completed_spans().is_empty());
    }
}
