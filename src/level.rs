#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Must be manually enabled inside the crate
    TracePrivate,
    /// Available in crate-only debug builds, needs some work to ship to users
    DebugInternal,
    /// Shipped to users in their debug builds
    Info,
    /// In debug builds, logs.  In release builds, may be interesting to phone home
    Analytics,
    /// Shipped to users in their release builds
    Warning,
    /// Shipped to users in their release builds, runtime error
    Error,
    /// Shipped to users in their release builds, programmer error
    Panic,
}