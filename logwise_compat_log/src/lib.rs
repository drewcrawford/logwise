// SPDX-License-Identifier: MIT OR Apache-2.0

//! Privacy-conservative ingress from the [`log`] facade.
//!
//! Every imported value is `LocalOnly`, and imported records use
//! [`logwise::Kind::AdHocText`]. The standard runtime therefore cannot route
//! them to a remote sink. This crate is optional and does not appear in the
//! dependency tree of the `logwise` facade.
//!
//! This crate intentionally provides no outbound adapter. Forwarding logwise
//! events back to `log` would lose privacy, context, links, detail tiers, and
//! stable schema information; an application that nevertheless adds such a
//! sink must treat the conversion as lossy.

use std::cell::Cell;
use std::fmt;

use log::kv::{Key, Source, Value, VisitSource};
use log::{Level, LevelFilter, Log, Metadata as LogMetadata, Record, SetLoggerError};
use logwise::{
    Callsite, Class, Detail, Domain, EventRef, FieldMetadata, FieldRef, Interest, Kind, Metadata,
    Privacy, Severity, ValueRef,
};

static FIELDS: &[FieldMetadata] = &[
    FieldMetadata::new("origin", Privacy::LocalOnly, Detail::Core),
    FieldMetadata::new("target", Privacy::LocalOnly, Detail::Core),
    FieldMetadata::new("module", Privacy::LocalOnly, Detail::Core),
    FieldMetadata::new("file", Privacy::LocalOnly, Detail::Core),
    FieldMetadata::new("line", Privacy::LocalOnly, Detail::Core),
    FieldMetadata::new("message", Privacy::LocalOnly, Detail::Core),
    FieldMetadata::new("fields", Privacy::LocalOnly, Detail::Core),
];

macro_rules! callsite {
    ($metadata:ident, $callsite:ident, $name:literal, $severity:ident) => {
        static $metadata: Metadata = Metadata {
            event_name: $name,
            package: "logwise_compat_log",
            target: "foreign.log",
            module: "logwise_compat_log",
            domain: Some(Domain::new("foreign.log")),
            severity: Severity::$severity,
            class: Class::Diagnostic,
            kind: Kind::AdHocText,
            location: None,
            fields: FIELDS,
        };
        static $callsite: Callsite = Callsite::new(&$metadata);
    };
}

callsite!(TRACE_METADATA, TRACE_CALLSITE, "foreign.log.trace", Trace);
callsite!(DEBUG_METADATA, DEBUG_CALLSITE, "foreign.log.debug", Debug);
callsite!(INFO_METADATA, INFO_CALLSITE, "foreign.log.info", Info);
callsite!(WARN_METADATA, WARN_CALLSITE, "foreign.log.warn", Warn);
callsite!(ERROR_METADATA, ERROR_CALLSITE, "foreign.log.error", Error);

thread_local! {
    static IN_BRIDGE: Cell<bool> = const { Cell::new(false) };
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

/// A [`log::Log`] implementation that synchronously imports borrowed records.
#[derive(Clone, Copy, Debug, Default)]
pub struct LogwiseLogger;

impl LogwiseLogger {
    pub const fn new() -> Self {
        Self
    }
}

impl Log for LogwiseLogger {
    fn enabled(&self, metadata: &LogMetadata<'_>) -> bool {
        !BridgeGuard::active() && selected(metadata.level()).1.interest().any()
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let Some(_guard) = BridgeGuard::enter() else {
            return;
        };
        let (metadata, callsite) = selected(record.level());
        let cached = callsite.interest();
        let context = logwise::context::capture();
        let interest = callsite.contextual_interest(cached, context);
        if !interest.wants(Privacy::LocalOnly, Detail::Core) {
            return;
        }

        let values = KeyValues(record.key_values());
        let fields = [
            local(0, ValueRef::Str("log"), interest),
            local(1, ValueRef::Str(record.target()), interest),
            local(
                2,
                ValueRef::Str(record.module_path().unwrap_or("")),
                interest,
            ),
            local(3, ValueRef::Str(record.file().unwrap_or("")), interest),
            local(
                4,
                ValueRef::U64(record.line().unwrap_or(0).into()),
                interest,
            ),
            local(5, ValueRef::display(record.args()), interest),
            local(6, ValueRef::display(&values), interest),
        ];
        callsite.emit(EventRef::structured(metadata, context, &fields));
    }

    fn flush(&self) {}
}

fn local<'a>(index: usize, value: ValueRef<'a>, interest: Interest) -> Option<FieldRef<'a>> {
    interest
        .wants(Privacy::LocalOnly, Detail::Core)
        .then(|| FieldRef::new(&FIELDS[index], value))
}

fn selected(level: Level) -> (&'static Metadata, &'static Callsite) {
    match level {
        Level::Trace => (&TRACE_METADATA, &TRACE_CALLSITE),
        Level::Debug => (&DEBUG_METADATA, &DEBUG_CALLSITE),
        Level::Info => (&INFO_METADATA, &INFO_CALLSITE),
        Level::Warn => (&WARN_METADATA, &WARN_CALLSITE),
        Level::Error => (&ERROR_METADATA, &ERROR_CALLSITE),
    }
}

struct KeyValues<'a>(&'a dyn Source);

impl fmt::Display for KeyValues<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Visitor<'a, 'b> {
            formatter: &'a mut fmt::Formatter<'b>,
            first: bool,
            rendered: fmt::Result,
        }

        impl<'kvs> VisitSource<'kvs> for Visitor<'_, '_> {
            fn visit_pair(
                &mut self,
                key: Key<'kvs>,
                value: Value<'kvs>,
            ) -> Result<(), log::kv::Error> {
                if self.rendered.is_err() {
                    return Ok(());
                }
                if !self.first {
                    self.rendered = self.formatter.write_str(", ");
                }
                if self.rendered.is_err() {
                    return Ok(());
                }
                self.first = false;
                self.rendered = write!(self.formatter, "{key}={value:?}");
                Ok(())
            }
        }

        formatter.write_str("{")?;
        let mut visitor = Visitor {
            formatter,
            first: true,
            rendered: Ok(()),
        };
        self.0.visit(&mut visitor).map_err(|_| fmt::Error)?;
        visitor.rendered?;
        formatter.write_str("}")
    }
}

/// Installs the bridge as the process `log` logger and enables all levels.
///
/// Runtime filtering still occurs before a record is dispatched. Use
/// [`install_with_max_level`] when dependencies should be statically capped.
pub fn install() -> Result<(), SetLoggerError> {
    install_with_max_level(LevelFilter::Trace)
}

/// Installs the bridge with the requested `log` facade maximum level.
pub fn install_with_max_level(max_level: LevelFilter) -> Result<(), SetLoggerError> {
    log::set_logger(&LogwiseLogger)?;
    log::set_max_level(max_level);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use logwise::{ContextToken, Dispatch, EventRef, Metadata};

    use super::*;

    #[derive(Debug)]
    struct Captured {
        name: &'static str,
        kind: Kind,
        severity: Severity,
        fields: Vec<(&'static str, Privacy, String)>,
    }

    struct Capture {
        records: Mutex<Vec<Captured>>,
        reentrant_attempts: AtomicUsize,
    }

    impl Dispatch for Capture {
        fn generation(&self) -> usize {
            0
        }

        fn interest(&self, _metadata: &'static Metadata) -> Interest {
            Interest::CORE_LOCAL
        }

        fn emit(&self, event: EventRef<'_>) {
            let fields = event
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
            self.records.lock().unwrap().push(Captured {
                name: event.metadata.event_name,
                kind: event.metadata.kind,
                severity: event.metadata.severity,
                fields,
            });

            self.reentrant_attempts.fetch_add(1, Ordering::Relaxed);
            let nested = Record::builder()
                .args(format_args!("outbound loop"))
                .level(Level::Info)
                .target("logwise.outbound")
                .build();
            LogwiseLogger.log(&nested);
        }
    }

    static CAPTURE: Capture = Capture {
        records: Mutex::new(Vec::new()),
        reentrant_attempts: AtomicUsize::new(0),
    };

    #[test]
    fn imports_fields_locally_and_stops_reentrant_loops() {
        logwise::install_dispatcher(&CAPTURE).unwrap();
        let values = [("request_id", 42_u64)];
        let record = Record::builder()
            .args(format_args!("hello"))
            .level(Level::Warn)
            .target("dependency.target")
            .module_path(Some("dependency::module"))
            .file(Some("dependency.rs"))
            .line(Some(27))
            .key_values(&values)
            .build();

        LogwiseLogger.log(&record);

        let records = CAPTURE.records.lock().unwrap();
        assert_eq!(records.len(), 1, "reentrant log record must be dropped");
        let captured = &records[0];
        assert_eq!(captured.name, "foreign.log.warn");
        assert_eq!(captured.kind, Kind::AdHocText);
        assert_eq!(captured.severity, Severity::Warn);
        assert!(
            captured
                .fields
                .iter()
                .all(|field| field.1 == Privacy::LocalOnly)
        );
        assert!(
            captured
                .fields
                .iter()
                .any(|field| { field.0 == "target" && field.2.contains("dependency.target") })
        );
        assert!(captured.fields.iter().any(|field| {
            field.0 == "fields" && field.2.contains("request_id") && field.2.contains("42")
        }));
        assert_eq!(CAPTURE.reentrant_attempts.load(Ordering::Relaxed), 1);
        assert_eq!(logwise::context::capture(), ContextToken::NONE);
    }
}
