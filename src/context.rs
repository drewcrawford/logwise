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
