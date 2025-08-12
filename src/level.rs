//SPDX-License-Identifier: MIT OR Apache-2.0
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
