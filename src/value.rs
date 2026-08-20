// SPDX-License-Identifier: MIT OR Apache-2.0

//! Borrowed structured values, and the borrowing rule that keeps them free.
//!
//! A [`ValueRef`] borrows from the call site's own stack frame and is valid
//! only for the duration of the synchronous dispatch call. That is what makes
//! emitting an event allocation-free: nothing is copied on the way out. A
//! runtime that wants to *retain* an event must copy or serialize it before it
//! returns, and this module's lifetimes are what force that decision to be
//! made explicitly rather than by accident.
//!
//! [`EventRef`] bundles the static [`Metadata`](crate::Metadata) with the
//! borrowed fields and the optional ad-hoc message; [`FieldRef`] pairs one
//! value with its own privacy and detail policy.

use core::fmt;

use crate::{ContextToken, FieldMetadata, Metadata};

/// A borrowed structured value.
///
/// Not `#[non_exhaustive]`: a runtime must handle every representation to
/// serialize an event at all, so a new variant should break that match rather
/// than be silently dropped by a wildcard arm.
///
/// Values are valid only for the synchronous dispatch call. A runtime that
/// retains an event must copy or serialize the values before returning.
#[derive(Clone, Copy)]
pub enum ValueRef<'a> {
    /// A boolean.
    Bool(bool),
    /// A signed integer. Smaller signed types widen into this.
    I64(i64),
    /// An unsigned integer. Smaller unsigned types widen into this.
    U64(u64),
    /// A floating-point number.
    F64(f64),
    /// A borrowed string slice.
    Str(&'a str),
    /// A value borrowed through its `Debug` implementation, formatted only if
    /// some view actually retains it.
    Debug(&'a dyn fmt::Debug),
    /// A value borrowed through its `Display` implementation, formatted only
    /// if some view actually retains it.
    Display(&'a dyn fmt::Display),
}

impl<'a> ValueRef<'a> {
    /// Borrows a value through its `Debug` representation.
    pub const fn debug(value: &'a dyn fmt::Debug) -> Self {
        Self::Debug(value)
    }

    /// Borrows a value through its `Display` representation.
    pub const fn display(value: &'a dyn fmt::Display) -> Self {
        Self::Display(value)
    }
}

impl fmt::Debug for ValueRef<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(value) => value.fmt(formatter),
            Self::I64(value) => value.fmt(formatter),
            Self::U64(value) => value.fmt(formatter),
            Self::F64(value) => value.fmt(formatter),
            Self::Str(value) => value.fmt(formatter),
            Self::Debug(value) => value.fmt(formatter),
            Self::Display(value) => value.fmt(formatter),
        }
    }
}

impl From<bool> for ValueRef<'_> {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

macro_rules! signed_value {
    ($($ty:ty),+ $(,)?) => {
        $(impl From<$ty> for ValueRef<'_> {
            fn from(value: $ty) -> Self {
                Self::I64(value as i64)
            }
        })+
    };
}

macro_rules! unsigned_value {
    ($($ty:ty),+ $(,)?) => {
        $(impl From<$ty> for ValueRef<'_> {
            fn from(value: $ty) -> Self {
                Self::U64(value as u64)
            }
        })+
    };
}

signed_value!(i8, i16, i32, i64, isize);
unsigned_value!(u8, u16, u32, u64, usize);

impl From<f32> for ValueRef<'_> {
    fn from(value: f32) -> Self {
        Self::F64(value.into())
    }
}

impl From<f64> for ValueRef<'_> {
    fn from(value: f64) -> Self {
        Self::F64(value)
    }
}

impl<'a> From<&'a str> for ValueRef<'a> {
    fn from(value: &'a str) -> Self {
        Self::Str(value)
    }
}

/// One materialized field and its static policy.
#[derive(Clone, Copy, Debug)]
pub struct FieldRef<'a> {
    /// The field's static name and privacy/detail policy.
    pub metadata: &'static FieldMetadata,
    /// The borrowed value, valid for the dispatch call only.
    pub value: ValueRef<'a>,
}

impl<'a> FieldRef<'a> {
    /// Pairs a value with the field metadata describing it.
    pub const fn new(metadata: &'static FieldMetadata, value: ValueRef<'a>) -> Self {
        Self { metadata, value }
    }
}

/// A synchronous borrowed event delivered to the installed dispatcher.
#[derive(Clone, Copy, Debug)]
pub struct EventRef<'a> {
    /// The call site's static schema.
    pub metadata: &'static Metadata,
    /// The causal context entered when the event was emitted.
    pub context: ContextToken,
    /// Materialized fields in metadata order.
    ///
    /// Entries that no active view requested remain `None`, so call sites can
    /// construct the borrowed set on the stack without allocation.
    pub fields: &'a [Option<FieldRef<'a>>],
    /// Formatted text for an [`AdHocText`](crate::Kind::AdHocText) call site,
    /// and `None` for a structured one.
    pub message: Option<fmt::Arguments<'a>>,
}

impl<'a> EventRef<'a> {
    /// An event carrying typed fields.
    pub const fn structured(
        metadata: &'static Metadata,
        context: ContextToken,
        fields: &'a [Option<FieldRef<'a>>],
    ) -> Self {
        Self {
            metadata,
            context,
            fields,
            message: None,
        }
    }

    /// An event carrying opaque formatted text and no fields.
    ///
    /// Formatting has already erased the field boundaries, so a runtime must
    /// keep this out of any remote view regardless of the call site's privacy
    /// labels.
    pub const fn text(
        metadata: &'static Metadata,
        context: ContextToken,
        message: fmt::Arguments<'a>,
    ) -> Self {
        Self {
            metadata,
            context,
            fields: &[],
            message: Some(message),
        }
    }
}
