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

/// How serious an event is.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Severity {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Critical,
}

/// Why a call site was instrumented.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Class {
    Operational,
    Diagnostic,
    Forensic,
    Performance,
    Metric,
}

/// What shape of observation a call site emits.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Kind {
    Event,
    AdHocText,
    Span,
    Counter,
    Measurement,
}

/// Where a field is allowed to be observed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Privacy {
    SupportSafe,
    LocalOnly,
    Secret,
}

/// How expensive a field is allowed to be at the call site.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Detail {
    Core,
    Detail,
}

/// Developer-authored metadata for one dynamic field.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FieldMetadata {
    pub name: &'static str,
    pub privacy: Privacy,
    pub detail: Detail,
}

impl FieldMetadata {
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
    pub file: &'static str,
    pub line: u32,
    pub column: u32,
}

impl Location {
    pub const fn new(file: &'static str, line: u32, column: u32) -> Self {
        Self { file, line, column }
    }
}

/// An optional hierarchical domain override.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Domain {
    pub name: &'static str,
}

impl Domain {
    pub const fn new(name: &'static str) -> Self {
        Self { name }
    }
}

/// Static schema and source metadata for one call site.
#[derive(Debug)]
pub struct Metadata {
    pub event_name: &'static str,
    pub package: &'static str,
    pub target: &'static str,
    pub module: &'static str,
    pub domain: Option<Domain>,
    pub severity: Severity,
    pub class: Class,
    pub kind: Kind,
    pub location: Option<Location>,
    pub fields: &'static [FieldMetadata],
}
