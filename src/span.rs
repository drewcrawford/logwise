// SPDX-License-Identifier: MIT OR Apache-2.0

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
    pub const NONE: Self = Self { id: 0, flags: 0 };

    #[doc(hidden)]
    pub const fn from_parts(id: u64, flags: u64) -> Self {
        Self { id, flags }
    }

    #[doc(hidden)]
    pub const fn into_parts(self) -> (u64, u64) {
        (self.id, self.flags)
    }

    pub const fn is_none(self) -> bool {
        self.id == 0
    }
}

/// The distinct timing question answered by a span.
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
#[derive(Clone, Copy)]
pub struct SpanRef<'a> {
    pub event: EventRef<'a>,
    pub timing: SpanTiming,
    pub warning_threshold: Option<Duration>,
}

/// Ends a runtime-owned span on drop.
///
/// The active context is captured at construction, so completion remains
/// attached to the originating task even if another context is active later.
#[must_use = "dropping the guard ends the span"]
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
