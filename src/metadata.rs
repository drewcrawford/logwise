// SPDX-License-Identifier: MIT OR Apache-2.0

//! The static schema a call site carries, and the axes it is classified on.
//!
//! Every value here is `'static` and built at the call site, so describing an
//! event costs no work at runtime. The axes are deliberately independent:
//! [`Class`] says *why* a site was instrumented (operational, diagnostic,
//! forensic, performance, metric) while [`Severity`] says how serious this
//! particular occurrence is — the pair answers questions that a single
//! `debug`-versus-`info` scale cannot.
//!
//! [`Privacy`] is the axis sinks are gated on. It is a property of a field,
//! not of a destination, so a runtime can decide what a remote sink is
//! permitted to see without asking the call site to know where its data goes.
//! [`Detail`] is orthogonal again: it defers an expensive field expression
//! until an observer explicitly asks for it.
//!
//! The enums in this module are deliberately **not** `#[non_exhaustive]`.
//! They are the dispatch ABI: a runtime matches every variant exhaustively to
//! decide what it may retain, and adding a variant genuinely is a breaking
//! change that should stop that match from compiling rather than let it fall
//! through to a default. `#[non_exhaustive]` would convert a compile error we
//! want into a silently mishandled privacy tier.

/// How serious an event is.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Severity {
    /// Step-by-step detail, useful only when following one execution closely.
    Trace,
    /// Information for whoever is debugging this code right now.
    Debug,
    /// A normal occurrence worth a record in the ordinary course of running.
    Info,
    /// Something unexpected that the program handled and continued past.
    Warn,
    /// An operation failed. The program continues, but something did not work.
    Error,
    /// The program cannot continue correctly.
    Critical,
}

/// Why a call site was instrumented.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Class {
    /// A stable fact about what the program did, for whoever operates it.
    /// Operational events are always compiled in: they are how a failure is
    /// seen at all, so they cannot be feature-gated away.
    Operational,
    /// Detail for whoever is debugging the code.
    Diagnostic,
    /// Retained for after-the-fact reconstruction of how a state was reached.
    Forensic,
    /// A timing observation.
    Performance,
    /// An aggregate quantity, meaningful when summed or averaged rather than
    /// read one record at a time.
    Metric,
}

/// What shape of observation a call site emits.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Kind {
    /// A named occurrence with a stable schema of typed fields.
    Event,
    /// Opaque formatted text with no field structure. Because formatting
    /// erases field boundaries, this kind can never be relabelled safe for a
    /// remote destination.
    AdHocText,
    /// The opening or closing of a timed region.
    Span,
    /// An increment of a named quantity.
    Counter,
    /// A sampled value of a named quantity.
    Measurement,
}

/// Where a field is allowed to be observed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Privacy {
    /// Safe to leave the machine. Only these values are copied into the view
    /// a remote sink receives.
    SupportSafe,
    /// May be retained on this machine, but never sent anywhere.
    LocalOnly,
    /// Never retained at all. A secret field is materialized only for a sink
    /// explicitly trusted with it, and never written to a buffer.
    Secret,
}

/// How expensive a field is allowed to be at the call site.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Detail {
    /// Evaluated whenever any view wants the field's privacy group.
    Core,
    /// Left unevaluated until an observer explicitly asks for expensive
    /// detail. This is how a costly field can sit on a hot call site without
    /// costing anything when nobody is looking.
    Detail,
}

/// Developer-authored metadata for one dynamic field.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FieldMetadata {
    /// The field's name in this call site's schema.
    pub name: &'static str,
    /// Where this field's value is allowed to be observed.
    pub privacy: Privacy,
    /// Whether this field's expression may be deferred.
    pub detail: Detail,
}

impl FieldMetadata {
    /// Describes one dynamic field.
    pub const fn new(name: &'static str, privacy: Privacy, detail: Detail) -> Self {
        Self {
            name,
            privacy,
            detail,
        }
    }
}

/// Source location retained as ancillary call-site information.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Location {
    /// Source file, as `file!()` reports it.
    pub file: &'static str,
    /// Line within that file.
    pub line: u32,
    /// Column within that line.
    pub column: u32,
}

impl Location {
    /// Records a source position.
    pub const fn new(file: &'static str, line: u32, column: u32) -> Self {
        Self { file, line, column }
    }
}

/// An optional hierarchical domain override.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Domain {
    /// The dotted domain name, most general segment first.
    pub name: &'static str,
}

impl Domain {
    /// Names a domain. Prefer the [`domain!`](crate::domain) macro at a call
    /// site.
    pub const fn new(name: &'static str) -> Self {
        Self { name }
    }
}

/// Static schema and source metadata for one call site.
#[derive(Debug)]
pub struct Metadata {
    /// The stable name a runtime selects and aggregates this call site by.
    /// Unlike the source location, it survives the code being moved.
    pub event_name: &'static str,
    /// The package the call site was compiled in.
    pub package: &'static str,
    /// The `target` string, defaulting to the package.
    pub target: &'static str,
    /// The module path within that package.
    pub module: &'static str,
    /// A hierarchical override selectors can match on, when the call site
    /// declares one.
    pub domain: Option<Domain>,
    /// How serious this occurrence is.
    pub severity: Severity,
    /// Why the call site exists.
    pub class: Class,
    /// What shape of observation it emits.
    pub kind: Kind,
    /// Where in the source it is, when the call site retained that.
    pub location: Option<Location>,
    /// The schema of its dynamic fields, in call-site order.
    pub fields: &'static [FieldMetadata],
}
