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
/// Values are valid only for the synchronous dispatch call. A runtime that
/// retains an event must copy or serialize the values before returning.
#[derive(Clone, Copy)]
pub enum ValueRef<'a> {
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
    Str(&'a str),
    Debug(&'a dyn fmt::Debug),
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
    pub metadata: &'static FieldMetadata,
    pub value: ValueRef<'a>,
}

impl<'a> FieldRef<'a> {
    pub const fn new(metadata: &'static FieldMetadata, value: ValueRef<'a>) -> Self {
        Self { metadata, value }
    }
}

/// A synchronous borrowed event delivered to the installed dispatcher.
#[derive(Clone, Copy)]
pub struct EventRef<'a> {
    pub metadata: &'static Metadata,
    pub context: ContextToken,
    /// Materialized fields in metadata order.
    ///
    /// Entries that no active view requested remain `None`, so call sites can
    /// construct the borrowed set on the stack without allocation.
    pub fields: &'a [Option<FieldRef<'a>>],
    pub message: Option<fmt::Arguments<'a>>,
}

impl<'a> EventRef<'a> {
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
