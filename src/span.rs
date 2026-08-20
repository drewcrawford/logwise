// SPDX-License-Identifier: MIT OR Apache-2.0

//! Spans, and the three different timing questions they answer.
//!
//! [`SpanTiming`] is the point of this module. Wall time, active poll time and
//! wake latency are genuinely different measurements — an async task that
//! takes a second of wall time may have polled for a microsecond — and
//! collapsing them into one "duration" is how span data stops meaning
//! anything. A call site picks the question it is asking.
//!
//! [`SpanToken`] is runtime-owned identity, opaque here in the same way
//! [`ContextToken`](crate::ContextToken) is. [`SpanGuard`] closes the span on
//! drop, including while unwinding, so a span cannot be left open by an early
//! return or a panic.

use core::marker::PhantomData;
use core::time::Duration;

use crate::{ContextToken, EventRef, dispatch};

/// Opaque identity of a runtime-owned span.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[repr(C)]
pub struct SpanToken {
    id: u64,
    flags: u64,
}

impl SpanToken {
    /// The no-runtime value, identifying no span.
    pub const NONE: Self = Self { id: 0, flags: 0 };

    #[doc(hidden)]
    pub const fn from_parts(id: u64, flags: u64) -> Self {
        Self { id, flags }
    }

    #[doc(hidden)]
    pub const fn into_parts(self) -> (u64, u64) {
        (self.id, self.flags)
    }

    /// Whether this token identifies no span.
    pub const fn is_none(self) -> bool {
        self.id == 0
    }
}

/// The distinct timing question answered by a span.
///
/// Exhaustive for the same reason as [`Class`](crate::Class) and
/// [`Privacy`](crate::Privacy): a runtime that cannot answer one of these
/// questions should fail to compile, not report the wrong duration.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum SpanTiming {
    /// Creation through completion, including time spent waiting.
    WallTime,
    /// Time spent actively polling or executing work.
    ActiveTime,
    /// Time between a wake signal and the next poll.
    WakeLatency,
}

/// Borrowed span start delivered synchronously to the runtime.
#[derive(Clone, Copy, Debug)]
pub struct SpanRef<'a> {
    /// The opening event: schema, context and fields.
    pub event: EventRef<'a>,
    /// Which timing question this span answers.
    pub timing: SpanTiming,
    /// A duration past which the runtime should report the span as a
    /// performance warning when it closes.
    pub warning_threshold: Option<Duration>,
}

/// Ends a runtime-owned span on drop.
///
/// The active context is captured at construction, so completion remains
/// attached to the originating task even if another context is active later.
#[must_use = "dropping the guard ends the span"]
#[derive(Debug)]
pub struct SpanGuard {
    token: SpanToken,
    context: ContextToken,
    active: bool,
    not_send: PhantomData<*mut ()>,
}

impl SpanGuard {
    #[doc(hidden)]
    pub const fn disabled() -> Self {
        Self {
            token: SpanToken::NONE,
            context: ContextToken::NONE,
            active: false,
            not_send: PhantomData,
        }
    }

    pub(crate) const fn new(token: SpanToken, context: ContextToken) -> Self {
        Self {
            token,
            context,
            active: !token.is_none(),
            not_send: PhantomData,
        }
    }

    /// The runtime-owned identity of the span this guard will close.
    pub const fn token(&self) -> SpanToken {
        self.token
    }
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        if self.active {
            dispatch::end_span(self.token, self.context);
        }
    }
}
