// SPDX-License-Identifier: MIT OR Apache-2.0

//! Runtime-owned context storage and monotonic span timing for the facade.

use std::cell::Cell;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use logwise::{
    Class, ContextToken, Detail, Dispatch, EventRef, InstallError, Interest, Metadata, Privacy,
    Severity, SpanRef, SpanTiming, SpanToken, install_dispatcher,
};

use crate::projection::{Capability, DetailLevel, EventSink, ProjectedEvent, ProjectedField};
use crate::spinlock::Spinlock;
use crate::sys::{Duration, Instant};

std::thread_local! {
    static CURRENT_CONTEXT: Cell<ContextToken> = const { Cell::new(ContextToken::NONE) };
    static IN_DISPATCH: Cell<bool> = const { Cell::new(false) };
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

/// Platform constraint for an activation request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Native,
    Wasm,
}

/// Runtime selector over static metadata and causal context.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Filter {
    domain: Option<String>,
    event_name: Option<String>,
    class: Option<Class>,
    minimum_severity: Option<Severity>,
    context: Option<ContextToken>,
    descendants: bool,
    target: Option<Target>,
}

impl Filter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    pub fn event(mut self, event_name: impl Into<String>) -> Self {
        self.event_name = Some(event_name.into());
        self
    }

    pub const fn class(mut self, class: Class) -> Self {
        self.class = Some(class);
        self
    }

    pub const fn minimum_severity(mut self, severity: Severity) -> Self {
        self.minimum_severity = Some(severity);
        self
    }

    pub const fn context(mut self, context: ContextToken, descendants: bool) -> Self {
        self.context = Some(context);
        self.descendants = descendants;
        self
    }

    pub const fn target(mut self, target: Target) -> Self {
        self.target = Some(target);
        self
    }
}

/// Result of asking the runtime to activate an observed selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationResult {
    Enabled,
    UnavailableTarget,
    NotCompiled,
    UnknownSelector,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SinkId(u64);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeDeliveryStats {
    pub sink_panics: u64,
    pub reentrant_events_dropped: u64,
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
    filter: Filter,
    interest: Interest,
    expires: Instant,
}

#[derive(Clone)]
struct SinkRegistration {
    id: SinkId,
    sink: Arc<dyn EventSink>,
    capability: Capability,
    detail: DetailLevel,
    filter: Filter,
}

#[derive(Default)]
struct State {
    contexts: HashMap<u64, ContextSnapshot>,
    active_spans: HashMap<u64, ActiveSpan>,
    completed_spans: Vec<CompletedSpan>,
    activations: Vec<Activation>,
    sinks: Vec<SinkRegistration>,
    catalog: Vec<&'static Metadata>,
}

/// The mutable runtime installed behind logwise's stable facade dispatcher.
pub struct Runtime {
    generation: AtomicUsize,
    baseline_interest: AtomicUsize,
    next_context: AtomicU64,
    next_span: AtomicU64,
    next_sink: AtomicU64,
    sink_panics: AtomicU64,
    reentrant_events_dropped: AtomicU64,
    state: Spinlock<State>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            generation: AtomicUsize::new(0),
            baseline_interest: AtomicUsize::new(Interest::NONE.bits()),
            next_context: AtomicU64::new(1),
            next_span: AtomicU64::new(1),
            next_sink: AtomicU64::new(1),
            sink_panics: AtomicU64::new(0),
            reentrant_events_dropped: AtomicU64::new(0),
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
        let _ = self.activate(Filter::new().context(root, true), interest, ttl);
    }

    /// Activates a selector over the observed call-site catalog.
    pub fn activate(&self, filter: Filter, interest: Interest, ttl: Duration) -> ActivationResult {
        if filter
            .target
            .is_some_and(|target| target != current_target())
        {
            return ActivationResult::UnavailableTarget;
        }

        let availability = self.selector_availability(&filter);
        if availability != ActivationResult::Enabled && filter.context.is_none() {
            return availability;
        }

        let expires = Instant::now() + ttl;
        self.state.with_mut(|state| {
            state.activations.push(Activation {
                filter,
                interest: interest.without_contextual(),
                expires,
            });
        });
        self.advance_generation();
        ActivationResult::Enabled
    }

    pub fn add_remote_sink(
        &self,
        sink: Arc<dyn EventSink>,
        filter: Filter,
        detail: DetailLevel,
    ) -> SinkId {
        self.add_sink(sink, Capability::Remote, filter, detail)
    }

    pub fn add_local_sink(
        &self,
        sink: Arc<dyn EventSink>,
        filter: Filter,
        detail: DetailLevel,
    ) -> SinkId {
        self.add_sink(sink, Capability::LocalRetained, filter, detail)
    }

    /// Registers an explicitly trusted synchronous view. This is the only sink
    /// capability that can receive secret fields.
    pub fn add_ephemeral_sink(
        &self,
        sink: Arc<dyn EventSink>,
        filter: Filter,
        detail: DetailLevel,
    ) -> SinkId {
        self.add_sink(sink, Capability::TrustedEphemeral, filter, detail)
    }

    pub fn remove_sink(&self, id: SinkId) -> bool {
        let removed = self.state.with_mut(|state| {
            state
                .sinks
                .iter()
                .position(|registration| registration.id == id)
                .map(|position| state.sinks.remove(position))
        });
        let did_remove = removed.is_some();
        if did_remove {
            self.advance_generation();
        }
        // Sink destructors are user code and may log. Drop only after the
        // configuration lock has been released.
        drop(removed);
        did_remove
    }

    pub fn catalog(&self) -> Vec<&'static Metadata> {
        self.state.with(|state| state.catalog.clone())
    }

    pub fn delivery_stats(&self) -> RuntimeDeliveryStats {
        RuntimeDeliveryStats {
            sink_panics: self.sink_panics.load(Ordering::Relaxed),
            reentrant_events_dropped: self.reentrant_events_dropped.load(Ordering::Relaxed),
        }
    }

    pub fn context_is_active(&self, context: ContextToken) -> bool {
        self.activation_interest(None, context).any()
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

    fn add_sink(
        &self,
        sink: Arc<dyn EventSink>,
        capability: Capability,
        filter: Filter,
        detail: DetailLevel,
    ) -> SinkId {
        let raw = self.next_sink.fetch_add(1, Ordering::Relaxed);
        assert_ne!(raw, u64::MAX, "logwise sink IDs exhausted");
        let id = SinkId(raw);
        self.state.with_mut(|state| {
            state.sinks.push(SinkRegistration {
                id,
                sink,
                capability,
                detail,
                filter,
            });
        });
        self.advance_generation();
        id
    }

    fn selector_availability(&self, filter: &Filter) -> ActivationResult {
        self.state.with(|state| {
            if state
                .catalog
                .iter()
                .any(|metadata| filter.matches_static(metadata))
            {
                return ActivationResult::Enabled;
            }
            if filter.event_name.is_some()
                && state.catalog.iter().any(|metadata| {
                    filter
                        .domain
                        .as_deref()
                        .is_none_or(|domain| domain_matches(metadata, domain))
                })
            {
                ActivationResult::NotCompiled
            } else {
                ActivationResult::UnknownSelector
            }
        })
    }

    fn activation_interest(
        &self,
        metadata: Option<&'static Metadata>,
        context: ContextToken,
    ) -> Interest {
        let now = Instant::now();
        let (interest, removed_expired) = self.state.with_mut(|state| {
            let before = state.activations.len();
            state
                .activations
                .retain(|activation| activation.expires > now);
            let mut interest = Interest::NONE;
            for activation in &state.activations {
                if metadata.is_none_or(|metadata| activation.filter.matches_static(metadata))
                    && activation.filter.matches_context(&state.contexts, context)
                {
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

    fn prune_expired_activations(&self) {
        if self.state.with(|state| state.activations.is_empty()) {
            return;
        }
        let now = Instant::now();
        let removed = self.state.with_mut(|state| {
            let before = state.activations.len();
            state
                .activations
                .retain(|activation| activation.expires > now);
            before != state.activations.len()
        });
        if removed {
            self.advance_generation();
        }
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

impl Filter {
    fn matches_static(&self, metadata: &'static Metadata) -> bool {
        self.target.is_none_or(|target| target == current_target())
            && self
                .domain
                .as_deref()
                .is_none_or(|domain| domain_matches(metadata, domain))
            && self
                .event_name
                .as_deref()
                .is_none_or(|event| hierarchy_matches(metadata.event_name, event))
            && self.class.is_none_or(|class| metadata.class == class)
            && self
                .minimum_severity
                .is_none_or(|severity| metadata.severity as u8 >= severity as u8)
    }

    fn matches_context(
        &self,
        contexts: &HashMap<u64, ContextSnapshot>,
        context: ContextToken,
    ) -> bool {
        let Some(selected) = self.context else {
            return true;
        };
        if self.descendants {
            is_descendant(contexts, context, selected)
        } else {
            context == selected
        }
    }
}

fn hierarchy_matches(value: &str, selector: &str) -> bool {
    value == selector
        || value
            .strip_prefix(selector)
            .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with("::"))
}

fn domain_matches(metadata: &Metadata, selector: &str) -> bool {
    metadata
        .domain
        .is_some_and(|domain| hierarchy_matches(domain.name, selector))
        || hierarchy_matches(metadata.package, selector)
        || hierarchy_matches(metadata.target, selector)
        || hierarchy_matches(metadata.module, selector)
}

const fn current_target() -> Target {
    if cfg!(target_arch = "wasm32") {
        Target::Wasm
    } else {
        Target::Native
    }
}

fn sink_interest(capability: Capability, detail: DetailLevel) -> Interest {
    let core = match capability {
        Capability::Remote => Interest::CORE_SUPPORT,
        Capability::LocalRetained => Interest::CORE_SUPPORT.union(Interest::CORE_LOCAL),
        Capability::TrustedEphemeral => Interest::CORE_SUPPORT
            .union(Interest::CORE_LOCAL)
            .union(Interest::CORE_SECRET),
    };
    if detail == DetailLevel::Core {
        return core;
    }
    core.union(match capability {
        Capability::Remote => Interest::DETAIL_SUPPORT,
        Capability::LocalRetained => Interest::DETAIL_SUPPORT.union(Interest::DETAIL_LOCAL),
        Capability::TrustedEphemeral => Interest::DETAIL_SUPPORT
            .union(Interest::DETAIL_LOCAL)
            .union(Interest::DETAIL_SECRET),
    })
}

fn privacy_allowed(capability: Capability, privacy: Privacy) -> bool {
    match capability {
        Capability::Remote => privacy == Privacy::SupportSafe,
        Capability::LocalRetained => privacy != Privacy::Secret,
        Capability::TrustedEphemeral => true,
    }
}

fn project_event(
    event: EventRef<'_>,
    capability: Capability,
    detail: DetailLevel,
) -> ProjectedEvent<'_> {
    let fields: Vec<_> = event
        .fields
        .iter()
        .flatten()
        .filter(|field| {
            privacy_allowed(capability, field.metadata.privacy)
                && (detail == DetailLevel::Full || field.metadata.detail == Detail::Core)
        })
        .map(|field| ProjectedField {
            name: field.metadata.name,
            value: field.value,
        })
        .collect();
    let omitted_fields = event.metadata.fields.len().saturating_sub(fields.len());
    ProjectedEvent {
        metadata: event.metadata,
        context: event.context,
        fields,
        message: (capability != Capability::Remote)
            .then_some(event.message)
            .flatten(),
        omitted_fields,
    }
}

impl Dispatch for Runtime {
    fn generation(&self) -> usize {
        self.generation.load(Ordering::Acquire)
    }

    fn interest(&self, metadata: &'static Metadata) -> Interest {
        self.prune_expired_activations();
        let mut interest = Interest::from_bits(self.baseline_interest.load(Ordering::Acquire));
        self.state.with_mut(|state| {
            if !state
                .catalog
                .iter()
                .any(|known| core::ptr::eq(*known, metadata))
            {
                state.catalog.push(metadata);
            }
            for sink in &state.sinks {
                if sink.filter.matches_static(metadata) {
                    interest = interest.union(if sink.filter.context.is_some() {
                        Interest::CONTEXTUAL
                    } else {
                        sink_interest(sink.capability, sink.detail)
                    });
                }
            }
            for activation in &state.activations {
                if activation.filter.matches_static(metadata) {
                    // TTL activation is always refined dynamically. Otherwise
                    // a direct field mask could remain in the call-site cache
                    // after its deadline with no event to advance generation.
                    interest = interest.union(Interest::CONTEXTUAL);
                }
            }
        });
        interest
    }

    fn contextual_interest(&self, metadata: &'static Metadata, context: ContextToken) -> Interest {
        let mut interest = Interest::from_bits(self.baseline_interest.load(Ordering::Acquire))
            .union(self.activation_interest(Some(metadata), context));
        self.state.with(|state| {
            for sink in &state.sinks {
                if sink.filter.matches_static(metadata)
                    && sink.filter.matches_context(&state.contexts, context)
                {
                    interest = interest.union(sink_interest(sink.capability, sink.detail));
                }
            }
        });
        interest
    }

    fn emit(&self, event: EventRef<'_>) {
        if IN_DISPATCH.replace(true) {
            self.reentrant_events_dropped
                .fetch_add(1, Ordering::Relaxed);
            return;
        }
        struct ResetDispatch;
        impl Drop for ResetDispatch {
            fn drop(&mut self) {
                IN_DISPATCH.set(false);
            }
        }
        let _reset = ResetDispatch;

        let sinks: Vec<_> = self.state.with(|state| {
            state
                .sinks
                .iter()
                .filter(|sink| {
                    !(event.metadata.kind == logwise::Kind::AdHocText
                        && sink.capability == Capability::Remote)
                        && sink.filter.matches_static(event.metadata)
                        && sink.filter.matches_context(&state.contexts, event.context)
                })
                .cloned()
                .collect()
        });
        for sink in sinks {
            if catch_unwind(AssertUnwindSafe(|| {
                sink.sink
                    .emit(project_event(event, sink.capability, sink.detail));
            }))
            .is_err()
            {
                self.sink_panics.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

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

        let isolated = Runtime::new();
        assert_eq!(isolated.interest(&METADATA), Interest::NONE);
        assert_eq!(
            isolated.activate(
                Filter::new().event(METADATA.event_name),
                Interest::DETAIL_LOCAL,
                Duration::ZERO,
            ),
            ActivationResult::Enabled
        );
        assert_eq!(isolated.interest(&METADATA), Interest::NONE);
    }
}
