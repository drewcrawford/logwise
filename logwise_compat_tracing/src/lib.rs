// SPDX-License-Identifier: MIT OR Apache-2.0

//! Privacy-conservative ingress from [`tracing`].
//!
//! The [`LogwiseLayer`] maps tracing span parentage onto logwise context tokens,
//! maps `follows_from` relationships onto context links, and imports span/event
//! fields as opaque local-only text. Imported records use
//! [`logwise::Kind::AdHocText`], so the standard runtime never projects them to
//! remote sinks.
//!
//! There is deliberately no outbound layer. Such a layer would be lossy:
//! tracing has no native equivalent for logwise privacy, retention, detail
//! tiers, or sink-specific projection.

use std::cell::{Cell, RefCell};
use std::fmt::{self, Write};

use logwise::{
    Callsite, Class, ContextGuard, ContextToken, Detail, Domain, EventRef, FieldMetadata, FieldRef,
    Interest, Kind, Metadata, Privacy, Severity, ValueRef,
};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::{LookupSpan, Registry};

static FIELDS: &[FieldMetadata] = &[
    FieldMetadata::new("origin", Privacy::LocalOnly, Detail::Core),
    FieldMetadata::new("operation", Privacy::LocalOnly, Detail::Core),
    FieldMetadata::new("target", Privacy::LocalOnly, Detail::Core),
    FieldMetadata::new("name", Privacy::LocalOnly, Detail::Core),
    FieldMetadata::new("tracing_id", Privacy::LocalOnly, Detail::Core),
    FieldMetadata::new("parent_id", Privacy::LocalOnly, Detail::Core),
    FieldMetadata::new("fields", Privacy::LocalOnly, Detail::Core),
];

macro_rules! callsite {
    ($metadata:ident, $callsite:ident, $name:literal, $severity:ident) => {
        static $metadata: Metadata = Metadata {
            event_name: $name,
            package: "logwise_compat_tracing",
            target: "foreign.tracing",
            module: "logwise_compat_tracing",
            domain: Some(Domain::new("foreign.tracing")),
            severity: Severity::$severity,
            class: Class::Diagnostic,
            kind: Kind::AdHocText,
            location: None,
            fields: FIELDS,
        };
        static $callsite: Callsite = Callsite::new(&$metadata);
    };
}

callsite!(
    TRACE_METADATA,
    TRACE_CALLSITE,
    "foreign.tracing.trace",
    Trace
);
callsite!(
    DEBUG_METADATA,
    DEBUG_CALLSITE,
    "foreign.tracing.debug",
    Debug
);
callsite!(INFO_METADATA, INFO_CALLSITE, "foreign.tracing.info", Info);
callsite!(WARN_METADATA, WARN_CALLSITE, "foreign.tracing.warn", Warn);
callsite!(
    ERROR_METADATA,
    ERROR_CALLSITE,
    "foreign.tracing.error",
    Error
);

thread_local! {
    static IN_BRIDGE: Cell<bool> = const { Cell::new(false) };
    static ENTERED: RefCell<Vec<(u64, ContextGuard)>> = const { RefCell::new(Vec::new()) };
}

struct BridgeGuard;

impl BridgeGuard {
    fn enter() -> Option<Self> {
        IN_BRIDGE.with(|active| (!active.replace(true)).then_some(Self))
    }

    fn active() -> bool {
        IN_BRIDGE.with(Cell::get)
    }
}

impl Drop for BridgeGuard {
    fn drop(&mut self) {
        IN_BRIDGE.with(|active| active.set(false));
    }
}

#[derive(Clone, Copy)]
struct SpanState {
    context: ContextToken,
    tracing_id: u64,
    parent_id: u64,
}

/// A composable tracing subscriber layer that imports into logwise.
#[derive(Clone, Copy, Debug, Default)]
pub struct LogwiseLayer;

impl LogwiseLayer {
    pub const fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for LogwiseLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(_bridge) = BridgeGuard::enter() else {
            return;
        };
        let parent = parent_state(attrs, &ctx);
        let context = logwise::context::child(
            parent.map_or(ContextToken::NONE, |state| state.context),
            attrs.metadata().name(),
        );
        let state = SpanState {
            context,
            tracing_id: id.into_u64(),
            parent_id: parent.map_or(0, |state| state.tracing_id),
        };
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(state);
        }
        let _entered = logwise::context::enter(context);
        if !wants_local(attrs.metadata().level()) {
            return;
        }
        let mut fields = FieldVisitor::default();
        attrs.record(&mut fields);
        emit(
            attrs.metadata().level(),
            "span.new",
            attrs.metadata().target(),
            attrs.metadata().name(),
            state.tracing_id,
            state.parent_id,
            &fields.output,
        );
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let Some(_bridge) = BridgeGuard::enter() else {
            return;
        };
        let Some((state, metadata)) = span_state_and_metadata(&ctx, id) else {
            return;
        };
        let _entered = logwise::context::enter(state.context);
        if !wants_local(metadata.level()) {
            return;
        }
        let mut fields = FieldVisitor::default();
        values.record(&mut fields);
        emit(
            metadata.level(),
            "span.record",
            metadata.target(),
            metadata.name(),
            state.tracing_id,
            state.parent_id,
            &fields.output,
        );
    }

    fn on_follows_from(&self, id: &Id, follows: &Id, ctx: Context<'_, S>) {
        let Some(_bridge) = BridgeGuard::enter() else {
            return;
        };
        let Some((state, metadata)) = span_state_and_metadata(&ctx, id) else {
            return;
        };
        let Some(related) = span_state(&ctx, follows) else {
            return;
        };
        logwise::context::link(state.context, related.context);
        let _entered = logwise::context::enter(state.context);
        emit(
            metadata.level(),
            "span.link",
            metadata.target(),
            metadata.name(),
            state.tracing_id,
            related.tracing_id,
            "",
        );
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let Some(_bridge) = BridgeGuard::enter() else {
            return;
        };
        let parent = ctx.event_span(event).and_then(|span| {
            let extensions = span.extensions();
            extensions.get::<SpanState>().copied()
        });
        let _entered = parent.map(|state| logwise::context::enter(state.context));
        if !wants_local(event.metadata().level()) {
            return;
        }
        let mut fields = FieldVisitor::default();
        event.record(&mut fields);
        emit(
            event.metadata().level(),
            "event",
            event.metadata().target(),
            event.metadata().name(),
            0,
            parent.map_or(0, |state| state.tracing_id),
            &fields.output,
        );
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        if BridgeGuard::active() {
            return;
        }
        if let Some(state) = span_state(&ctx, id) {
            ENTERED.with(|entered| {
                entered
                    .borrow_mut()
                    .push((id.into_u64(), logwise::context::enter(state.context)));
            });
        }
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        if BridgeGuard::active() {
            return;
        }
        ENTERED.with(|entered| {
            let mut entered = entered.borrow_mut();
            let Some(position) = entered.iter().rposition(|entry| entry.0 == id.into_u64()) else {
                // Entered on another thread, or never entered at all. Leave a
                // stack we do not own alone.
                return;
            };
            // `Entered` guards are ordinary values, so a span can be exited
            // while spans entered after it are still entered. Each logwise
            // guard restores the token that was current when it was created,
            // which means the stack only comes apart correctly from the top
            // down -- popping just this span's entry would strand every guard
            // above it and leave a finished span's context entered for the rest
            // of the thread's life.
            let mut still_entered = Vec::new();
            while entered.len() > position + 1 {
                let (above, guard) = entered.pop().expect("length checked above");
                drop(guard);
                still_entered.push(above);
            }
            entered.pop();
            // Those spans have not been exited yet, so put them back in the
            // order they were entered, rebuilding the restoration chain.
            for above in still_entered.into_iter().rev() {
                let Some(state) = span_state(&ctx, &Id::from_u64(above)) else {
                    continue;
                };
                entered.push((above, logwise::context::enter(state.context)));
            }
        });
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(_bridge) = BridgeGuard::enter() else {
            return;
        };
        let Some((state, metadata)) = span_state_and_metadata(&ctx, &id) else {
            return;
        };
        let _entered = logwise::context::enter(state.context);
        emit(
            metadata.level(),
            "span.close",
            metadata.target(),
            metadata.name(),
            state.tracing_id,
            state.parent_id,
            "",
        );
    }
}

fn parent_state<S>(attrs: &Attributes<'_>, ctx: &Context<'_, S>) -> Option<SpanState>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    if attrs.is_root() {
        None
    } else if let Some(parent) = attrs.parent() {
        span_state(ctx, parent)
    } else {
        ctx.lookup_current().and_then(|span| {
            let extensions = span.extensions();
            extensions.get::<SpanState>().copied()
        })
    }
}

fn span_state<S>(ctx: &Context<'_, S>, id: &Id) -> Option<SpanState>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    let span = ctx.span(id)?;
    let extensions = span.extensions();
    extensions.get::<SpanState>().copied()
}

fn span_state_and_metadata<'a, S>(
    ctx: &'a Context<'_, S>,
    id: &Id,
) -> Option<(SpanState, &'static tracing::Metadata<'static>)>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    let span = ctx.span(id)?;
    let state = {
        let extensions = span.extensions();
        extensions.get::<SpanState>().copied()?
    };
    Some((state, span.metadata()))
}

#[derive(Default)]
struct FieldVisitor {
    output: String,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if !self.output.is_empty() {
            self.output.push_str(", ");
        }
        let _ = write!(self.output, "{}={value:?}", field.name());
    }
}

#[allow(clippy::too_many_arguments)]
fn emit(
    level: &Level,
    operation: &str,
    target: &str,
    name: &str,
    tracing_id: u64,
    parent_id: u64,
    imported_fields: &str,
) {
    let (metadata, callsite) = selected(level);
    let cached = callsite.interest();
    if !cached.any() {
        return;
    }
    let context = logwise::context::capture();
    let interest = callsite.contextual_interest(cached, context);
    if !interest.wants(Privacy::LocalOnly, Detail::Core) {
        return;
    }
    let fields = [
        local(0, ValueRef::Str("tracing"), interest),
        local(1, ValueRef::Str(operation), interest),
        local(2, ValueRef::Str(target), interest),
        local(3, ValueRef::Str(name), interest),
        local(4, ValueRef::U64(tracing_id), interest),
        local(5, ValueRef::U64(parent_id), interest),
        local(6, ValueRef::Str(imported_fields), interest),
    ];
    callsite.emit(EventRef::structured(metadata, context, &fields));
}

fn local<'a>(index: usize, value: ValueRef<'a>, interest: Interest) -> Option<FieldRef<'a>> {
    interest
        .wants(Privacy::LocalOnly, Detail::Core)
        .then(|| FieldRef::new(&FIELDS[index], value))
}

fn wants_local(level: &Level) -> bool {
    let (_, callsite) = selected(level);
    let cached = callsite.interest();
    if !cached.any() {
        return false;
    }
    let context = logwise::context::capture();
    callsite
        .contextual_interest(cached, context)
        .wants(Privacy::LocalOnly, Detail::Core)
}

fn selected(level: &Level) -> (&'static Metadata, &'static Callsite) {
    match *level {
        Level::TRACE => (&TRACE_METADATA, &TRACE_CALLSITE),
        Level::DEBUG => (&DEBUG_METADATA, &DEBUG_CALLSITE),
        Level::INFO => (&INFO_METADATA, &INFO_CALLSITE),
        Level::WARN => (&WARN_METADATA, &WARN_CALLSITE),
        Level::ERROR => (&ERROR_METADATA, &ERROR_CALLSITE),
    }
}

/// Installs a registry containing only [`LogwiseLayer`] as the global tracing
/// subscriber.
///
/// Applications that already compose a subscriber should add
/// `LogwiseLayer::new()` themselves instead.
pub fn install() -> Result<(), tracing::subscriber::SetGlobalDefaultError> {
    tracing::subscriber::set_global_default(Registry::default().with(LogwiseLayer))
}

#[cfg(test)]
mod tests {
    use std::fmt;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    use super::*;
    use logwise::{Dispatch, EventRef, Metadata, SpanRef, SpanToken};

    thread_local! {
        static CURRENT: Cell<ContextToken> = const { Cell::new(ContextToken::NONE) };
    }

    #[derive(Debug)]
    struct Captured {
        operation: String,
        context: ContextToken,
        fields: Vec<(&'static str, Privacy, String)>,
    }

    struct Capture {
        generation: AtomicUsize,
        interest: AtomicUsize,
        next_context: AtomicU64,
        records: Mutex<Vec<Captured>>,
        parents: Mutex<Vec<(ContextToken, ContextToken)>>,
        links: Mutex<Vec<(ContextToken, ContextToken)>>,
    }

    impl Dispatch for Capture {
        fn generation(&self) -> usize {
            self.generation.load(Ordering::Relaxed)
        }

        fn interest(&self, _metadata: &'static Metadata) -> Interest {
            Interest::from_bits(self.interest.load(Ordering::Relaxed))
        }

        fn emit(&self, event: EventRef<'_>) {
            let fields: Vec<_> = event
                .fields
                .iter()
                .flatten()
                .map(|field| {
                    (
                        field.metadata.name,
                        field.metadata.privacy,
                        format!("{:?}", field.value),
                    )
                })
                .collect();
            let operation = fields
                .iter()
                .find(|field| field.0 == "operation")
                .map_or_else(String::new, |field| field.2.clone());
            self.records.lock().unwrap().push(Captured {
                operation,
                context: event.context,
                fields,
            });

            // Simulates a lossy outbound tracing sink. The layer's guard must
            // prevent this from cycling back into logwise.
            tracing::debug!(target: "logwise.outbound", "loop attempt");
        }

        fn capture_context(&self) -> ContextToken {
            CURRENT.with(Cell::get)
        }

        fn create_context(&self, parent: ContextToken, _name: &'static str) -> ContextToken {
            let token =
                ContextToken::from_parts(self.next_context.fetch_add(1, Ordering::Relaxed) + 1, 0);
            self.parents.lock().unwrap().push((token, parent));
            token
        }

        fn link_context(&self, context: ContextToken, related: ContextToken) {
            self.links.lock().unwrap().push((context, related));
        }

        fn enter_context(&self, context: ContextToken) -> ContextToken {
            CURRENT.with(|current| current.replace(context))
        }

        fn exit_context(&self, previous: ContextToken) {
            CURRENT.with(|current| current.set(previous));
        }

        fn start_span(&self, _span: SpanRef<'_>) -> SpanToken {
            SpanToken::NONE
        }
    }

    static CAPTURE: Capture = Capture {
        generation: AtomicUsize::new(0),
        interest: AtomicUsize::new(0),
        next_context: AtomicU64::new(0),
        records: Mutex::new(Vec::new()),
        parents: Mutex::new(Vec::new()),
        links: Mutex::new(Vec::new()),
    };

    static FORMATS: AtomicUsize = AtomicUsize::new(0);

    struct CountsFormats;

    impl fmt::Debug for CountsFormats {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            FORMATS.fetch_add(1, Ordering::Relaxed);
            formatter.write_str("formatted")
        }
    }

    #[test]
    fn maps_spans_parents_links_events_and_local_fields() {
        logwise::install_dispatcher(&CAPTURE).unwrap();

        let disabled_subscriber = Registry::default().with(LogwiseLayer);
        tracing::subscriber::with_default(disabled_subscriber, || {
            tracing::debug!(probe = ?CountsFormats, "disabled");
        });
        assert_eq!(FORMATS.load(Ordering::Relaxed), 0);

        CAPTURE
            .interest
            .store(Interest::CORE_LOCAL.bits(), Ordering::Relaxed);
        CAPTURE.generation.store(1, Ordering::Relaxed);
        let subscriber = Registry::default().with(LogwiseLayer);
        tracing::subscriber::with_default(subscriber, || {
            let root = tracing::info_span!("root", root_field = 1_u64);
            let related = tracing::debug_span!("related");
            let child = tracing::warn_span!(parent: &root, "child", child_field = "private");
            child.follows_from(&related);
            let _entered = child.enter();
            tracing::error!(answer = 42_u64, "failed");
            child.record("child_field", "updated");
        });

        let records = CAPTURE.records.lock().unwrap();
        assert_eq!(
            records.len(),
            9,
            "three span lifecycles, one link, one event, and one update only; outbound loop attempts must be dropped"
        );
        assert!(
            records
                .iter()
                .any(|record| record.operation.contains("span.new"))
        );
        let event = records
            .iter()
            .find(|record| record.operation.contains("event"))
            .unwrap();
        assert!(!event.context.is_none());
        assert!(
            event
                .fields
                .iter()
                .all(|field| field.1 == Privacy::LocalOnly)
        );
        assert!(event.fields.iter().any(|field| {
            field.0 == "fields" && field.2.contains("answer") && field.2.contains("42")
        }));

        let parents = CAPTURE.parents.lock().unwrap();
        assert_eq!(parents.len(), 3);
        assert!(parents.iter().any(|(_, parent)| !parent.is_none()));
        assert_eq!(CAPTURE.links.lock().unwrap().len(), 1);
    }
}
