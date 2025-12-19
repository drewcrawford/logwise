// SPDX-License-Identifier: MIT OR Apache-2.0

//! Log level enumeration for the logwise logging system.
//!
//! The `Level` enum defines the different severity levels available in logwise,
//! each with specific semantics about when they are active and where they should
//! be used. Unlike traditional logging systems, logwise uses opinionated levels
//! that correspond to specific use cases rather than generic severity indicators.

use std::fmt::Display;

/// Log severity levels with opinionated use-case semantics.
///
/// Unlike traditional logging systems with generic severity levels, logwise provides
/// specific levels for defined use cases. Each level has different availability
/// depending on build type (debug vs release) and runtime configuration.
///
/// # Level Hierarchy
///
/// The levels are ordered by severity from lowest to highest:
/// `Trace` < `DebugInternal` < `Info` < `Analytics` < `PerfWarn` < `Warning` < `Error` < `Panic` < `Mandatory` < `Profile`
///
/// # Build-Time Availability
///
/// | Level | Debug Builds | Release Builds |
/// |-------|--------------|----------------|
/// | `Trace` | ✓ (with tracing enabled) | ✗ |
/// | `DebugInternal` | ✓ | ✗ |
/// | `Info` | ✓ | ✗ |
/// | `Analytics` | ✓ | ✓ |
/// | `PerfWarn` | ✓ | ✓ |
/// | `Warning` | ✓ | ✓ |
/// | `Error` | ✓ | ✓ |
/// | `Panic` | ✓ | ✓ |
/// | `Mandatory` | ✓ | ✓ |
/// | `Profile` | ✓ | ✓ |
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Detailed debugging information, only available when tracing is explicitly enabled.
    ///
    /// Use for very verbose output that would be too noisy for normal debugging.
    /// Only available in debug builds when [`Context::begin_trace()`](crate::context::Context::begin_trace)
    /// has been called.
    ///
    /// # Example
    /// ```rust
    /// logwise::declare_logging_domain!();
    /// # fn main() {
    /// # use logwise::context::Context;
    /// Context::begin_trace();
    /// logwise::trace_sync!("Entering function with arg={val}", val=42);
    /// # }
    /// ```
    Trace,

    /// Print-style debugging for the library author.
    ///
    /// On by default in the current crate (when built with `--features logwise_internal`),
    /// but only available to downstream users in their debug builds with tracing enabled.
    /// Compiled out entirely in release builds.
    ///
    /// # Example
    /// ```rust
    /// logwise::declare_logging_domain!();
    /// # fn main() {
    /// logwise::debuginternal_sync!("Internal state: count={val}", val=42);
    /// # }
    /// ```
    DebugInternal,

    /// General information for downstream crate users.
    ///
    /// Available in debug builds, compiled out in release builds.
    /// Use for information that helps users understand what your library is doing.
    ///
    /// # Example
    /// ```rust
    /// logwise::declare_logging_domain!();
    /// # fn main() {
    /// logwise::info_sync!("Starting operation", name="data_sync");
    /// # }
    /// ```
    Info,

    /// Analytics and telemetry data.
    ///
    /// In debug builds, logs locally. In release builds, this data may be
    /// sent to analytics services. Use for metrics and usage data that help
    /// improve the software.
    Analytics,

    /// Performance warnings with timing analysis.
    ///
    /// Available in all builds. Use for operations that are unexpectedly slow
    /// and may need optimization. Integrates with the perfwarn interval system
    /// for automatic timing.
    ///
    /// # Example
    /// ```rust
    /// logwise::declare_logging_domain!();
    /// # fn main() {
    /// # fn slow_operation() {}
    /// logwise::perfwarn!("database_query", {
    ///     slow_operation();
    /// });
    /// # }
    /// ```
    PerfWarn,

    /// Suspicious conditions that may indicate problems.
    ///
    /// Available in all builds. Use when something unexpected happened but
    /// the operation can still continue.
    ///
    /// # Example
    /// ```rust
    /// logwise::declare_logging_domain!();
    /// # fn main() {
    /// logwise::warn_sync!("Config file not found, using defaults");
    /// # }
    /// ```
    Warning,

    /// Runtime errors that prevent normal operation.
    ///
    /// Available in all builds. Use when an operation failed due to external
    /// factors (network errors, invalid input, etc.).
    ///
    /// # Example
    /// ```rust
    /// logwise::declare_logging_domain!();
    /// # fn main() {
    /// logwise::error_sync!("Failed to connect: {reason}", reason="timeout");
    /// # }
    /// ```
    Error,

    /// Programmer errors indicating bugs in the code.
    ///
    /// Available in all builds. Use for conditions that should never occur
    /// if the code is correct (invariant violations, assertion failures).
    Panic,

    /// Printf-style debugging that is always enabled.
    ///
    /// Available in all builds including release. Use for temporary debugging
    /// in hostile environments where other log levels may be compiled out.
    /// Intended to be removed before committing code.
    ///
    /// # Example
    /// ```rust
    /// logwise::declare_logging_domain!();
    /// # fn main() {
    /// logwise::mandatory_sync!("Debug: value is {val}", val=42);
    /// # }
    /// ```
    Mandatory,

    /// Profiling output that is always enabled.
    ///
    /// Available in all builds including release. Use for temporary profiling
    /// and performance investigation. Intended to be removed before committing code.
    ///
    /// # Example
    /// ```rust
    /// logwise::declare_logging_domain!();
    /// # fn main() {
    /// let elapsed_ms = 42u64;
    /// logwise::profile_sync!("Operation took {ms} ms", ms=elapsed_ms);
    /// # }
    /// ```
    Profile,
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
            Level::Mandatory => write!(f, "MANDATORY"),
            Level::Profile => write!(f, "PROFILE"),
        }
    }
}
