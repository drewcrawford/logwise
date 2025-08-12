//SPDX-License-Identifier: MIT OR Apache-2.0

//! Log level enumeration for the logwise logging system.
//!
//! The `Level` enum defines the different severity levels available in logwise,
//! each with specific semantics about when they are active and where they should
//! be used. Unlike traditional logging systems, logwise uses opinionated levels
//! that correspond to specific use cases rather than generic severity indicators.

use std::fmt::Display;

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Only available in trace mode.
    Trace,
    /// Available by default within the current crate when compiled in debug mode.  Available to users in their debug builds with tracing enabled.
    DebugInternal,
    /// Shipped to users in their debug builds
    Info,
    /// In debug builds, logs.  In release builds, may be interesting to phone home
    Analytics,
    /**
    Available in release builds, provides performance-critical warnings.
    */
    PerfWarn,
    /// Shipped to users in their release builds
    Warning,
    /// Shipped to users in their release builds, runtime error
    Error,
    /// Shipped to users in their release builds, programmer error
    Panic,
}

// ============================================================================
// BOILERPLATE TRAIT IMPLEMENTATIONS
// ============================================================================
//
// Design decisions for Level trait implementations:
//
// IMPLEMENTED:
// - Debug/Clone/Copy: Already derived - appropriate for simple enum
// - PartialEq/Eq: Already derived - enables level comparison
// - PartialOrd/Ord: Already derived - establishes severity ordering (Trace < ... < Panic)
// - Hash: Already derived - consistent with Eq, enables use in hash collections
// - Display: Implemented - provides user-friendly string representation
// - #[non_exhaustive]: Applied - allows adding new levels without breaking changes
//
// NOT IMPLEMENTED:
// - Default: No obvious default level - application-specific choice
// - From/Into: No obvious conversions to/from other types
// - AsRef/AsMut: Enum doesn't naturally reference an underlying type
//
// AUTOMATIC:
// - Send/Sync: Automatically implemented - simple enum with no heap data

impl Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Level::Trace => write!(f, "TRACE"),
            Level::DebugInternal => write!(f, "DEBUG"),
            Level::Info => write!(f, "INFO"),
            Level::Analytics => write!(f, "ANALYTICS"),
            Level::PerfWarn => write!(f, "PERFWARN"),
            Level::Warning => write!(f, "WARN"),
            Level::Error => write!(f, "ERROR"),
            Level::Panic => write!(f, "PANIC"),
        }
    }
}
