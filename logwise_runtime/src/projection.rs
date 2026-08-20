// SPDX-License-Identifier: MIT OR Apache-2.0

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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
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
