// SPDX-License-Identifier: MIT OR Apache-2.0

//! Privacy projection: the only event shape a sink is ever handed.
//!
//! Sinks never see the facade's [`EventRef`](logwise::EventRef). The trusted
//! runtime reads it once, decides per field what this particular sink is
//! authorized to observe, and materializes a [`ProjectedEvent`] containing
//! only that. Local-only and secret values are not withheld from a remote sink
//! by convention — they are never copied into the view that remote code
//! receives, so there is no privacy boundary for a sink to cross.
//!
//! `omitted_fields` records how much a view did not get, so a sink can tell
//! "nothing was logged" apart from "you were not allowed to see it".

use core::fmt;

use logwise::{ContextToken, Detail, Metadata, Privacy, ValueRef};

/// A field already authorized for one sink view.
#[derive(Clone, Copy, Debug)]
pub struct ProjectedField<'a> {
    pub name: &'static str,
    pub privacy: Privacy,
    pub detail: Detail,
    pub value: ValueRef<'a>,
}

/// The only event representation exposed to runtime sinks.
///
/// The trusted runtime constructs this after privacy projection. Sink APIs do
/// not receive the raw facade `EventRef`.
#[derive(Debug)]
pub struct ProjectedEvent<'a> {
    pub metadata: &'static Metadata,
    pub context: ContextToken,
    pub fields: Vec<ProjectedField<'a>>,
    pub message: Option<fmt::Arguments<'a>>,
    pub omitted_fields: usize,
}

/// A synchronous destination for an already projected view.
pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, event: ProjectedEvent<'_>);
}

/// Whether a view asks call sites to evaluate expensive detail fields.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum DetailLevel {
    #[default]
    Core,
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Capability {
    Remote,
    LocalRetained,
    TrustedEphemeral,
}
