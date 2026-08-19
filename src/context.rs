// SPDX-License-Identifier: MIT OR Apache-2.0

/// Opaque causal context propagated by tasks and continuations.
///
/// IDs are minted by an installed runtime. The all-zero token is the no-runtime
/// value and carries no lineage.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[repr(C)]
pub struct ContextToken {
    id: u64,
    flags: u64,
}

impl ContextToken {
    /// The no-runtime/no-context token.
    pub const NONE: Self = Self { id: 0, flags: 0 };

    /// Constructs a token from runtime-owned opaque parts.
    ///
    /// This is public for runtime implementations, but application code should
    /// treat the representation as opaque.
    #[doc(hidden)]
    pub const fn from_parts(id: u64, flags: u64) -> Self {
        Self { id, flags }
    }

    /// Returns the runtime-owned opaque parts.
    #[doc(hidden)]
    pub const fn into_parts(self) -> (u64, u64) {
        (self.id, self.flags)
    }

    /// Whether this token represents no installed runtime context.
    pub const fn is_none(self) -> bool {
        self.id == 0
    }
}

/// Captures the context currently entered by the runtime.
pub fn capture() -> ContextToken {
    dispatch::capture_context()
}

/// Creates a durable child token with an explicit causal parent.
pub fn child(parent: ContextToken, name: &'static str) -> ContextToken {
    dispatch::create_context(parent, name)
}

/// Adds a non-parent causal link to an existing context.
pub fn link(context: ContextToken, related: ContextToken) {
    dispatch::link_context(context, related);
}

/// Enters a durable token for the current synchronous scope.
///
/// Custom executors should create/store a task token at spawn time, then enter
/// it immediately around every `Future::poll`.
#[must_use = "the guard restores the previous context when dropped"]
pub fn enter(context: ContextToken) -> ContextGuard {
    let Some(previous) = dispatch::enter_context(context) else {
        return ContextGuard::inactive();
    };
    ContextGuard {
        previous,
        active: true,
        not_send: PhantomData,
    }
}

/// Restores the previously entered context on drop.
#[must_use]
pub struct ContextGuard {
    previous: ContextToken,
    active: bool,
    // Entered state is thread/worker-local, so moving a live guard would make
    // its restoration target ambiguous.
    not_send: PhantomData<*mut ()>,
}

impl ContextGuard {
    const fn inactive() -> Self {
        Self {
            previous: ContextToken::NONE,
            active: false,
            not_send: PhantomData,
        }
    }
}

impl Drop for ContextGuard {
    fn drop(&mut self) {
        if self.active {
            dispatch::exit_context(self.previous);
        }
    }
}
use core::marker::PhantomData;

use crate::dispatch;
