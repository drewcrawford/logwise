// SPDX-License-Identifier: MIT OR Apache-2.0

//! Macro definitions and support types for the logwise logging library.
//!
//! This module provides the macros and supporting types used by the logging system.
//! The actual dispatch functions that handle log record creation and delivery are
//! in the [`dispatch`](crate::dispatch) module.
//!
//! # Architecture
//!
//! The logging flow follows this pattern:
//! 1. A `*_pre` function in [`dispatch`](crate::dispatch) creates a [`LogRecord`] with appropriate metadata
//! 2. The procedural macro uses [`PrivateFormatter`] to add the formatted message
//! 3. A `*_post` function in [`dispatch`](crate::dispatch) dispatches the record to all global loggers
//!
//! # Privacy
//!
//! The [`PrivateFormatter`] ensures that all logged values respect privacy constraints
//! by calling the [`Loggable::log_all`](crate::privacy::Loggable::log_all) method, which
//! includes all private information for local logging.
//!
//! # Example
//!
//! These functions are not intended to be called directly. Instead, use the macros:
//!
//! ```rust
//! logwise::declare_logging_domain!();
//! # fn main() {
//! # use logwise::context::Context;
//! # Context::reset("example".to_string());
//! // Use the macro, not the implementation functions
//! logwise::info_sync!("Operation completed", count=42);
//! # }
//! ```

use crate::log_record::LogRecord;
use crate::privacy::Loggable;
use std::sync::atomic::AtomicBool;

/// Controls whether internal logging is enabled for a crate.
///
/// This struct manages the internal logging domain for a crate, determining
/// whether `debuginternal` log messages are displayed. The domain is typically
/// configured at compile time based on the `logwise_internal` feature flag.
///
/// # Example
///
/// ```rust
/// # use logwise::LoggingDomain;
/// // Create a domain that's enabled
/// let domain = LoggingDomain::new(true);
/// assert!(domain.is_internal());
///
/// // Create a domain that's disabled
/// let domain = LoggingDomain::new(false);
/// assert!(!domain.is_internal());
/// ```
pub struct LoggingDomain {
    is_internal: AtomicBool,
}

impl LoggingDomain {
    /// Creates a new `LoggingDomain` with the specified internal logging state.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether internal logging should be enabled for this domain
    ///
    /// # Example
    ///
    /// ```rust
    /// # use logwise::LoggingDomain;
    /// let domain = LoggingDomain::new(true);
    /// ```
    #[inline]
    pub const fn new(enabled: bool) -> Self {
        Self {
            is_internal: AtomicBool::new(enabled),
        }
    }

    /// Returns whether internal logging is enabled for this domain.
    ///
    /// This determines whether `debuginternal` log messages will be displayed
    /// when logging from this crate.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use logwise::LoggingDomain;
    /// let domain = LoggingDomain::new(true);
    /// assert!(domain.is_internal());
    /// ```
    #[inline]
    pub fn is_internal(&self) -> bool {
        self.is_internal.load(std::sync::atomic::Ordering::Relaxed)
    }
}

// ============================================================================
// Boilerplate trait implementations for LoggingDomain
// ============================================================================

impl std::fmt::Debug for LoggingDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoggingDomain")
            .field("is_internal", &self.is_internal())
            .finish()
    }
}

impl Default for LoggingDomain {
    /// Creates a new `LoggingDomain` with internal logging disabled.
    ///
    /// This is the safe default for most use cases where internal logging
    /// should be explicitly enabled rather than accidentally left on.
    fn default() -> Self {
        Self::new(false)
    }
}

impl std::fmt::Display for LoggingDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LoggingDomain(internal={})", self.is_internal())
    }
}

impl From<bool> for LoggingDomain {
    /// Creates a `LoggingDomain` from a boolean value.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether internal logging should be enabled
    ///
    /// # Example
    ///
    /// ```rust
    /// # use logwise::LoggingDomain;
    /// let domain: LoggingDomain = true.into();
    /// assert!(domain.is_internal());
    /// ```
    fn from(enabled: bool) -> Self {
        Self::new(enabled)
    }
}

/// Compares two strings for equality at compile time.
///
/// This is used by [`declare_logging_domain!`] to determine if the current
/// compilation unit is the "current crate" (where `debuginternal` should be
/// enabled by default) or a downstream crate.
#[doc(hidden)]
pub const fn const_str_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Declares the logging domain for the current crate.
///
/// This macro sets up the internal logging configuration for a crate, determining
/// whether `debuginternal` log messages will be displayed. It must be declared at
/// the crate root (typically in `lib.rs` or `main.rs`) to enable the use of
/// `debuginternal_sync!` and `debuginternal_async!` macros.
///
/// # Two Invocation Patterns
///
/// ## 1. Default Invocation (Feature-Based)
///
/// When called without arguments, the macro automatically enables internal logging
/// based on the presence of the `logwise_internal` feature flag:
///
/// ```rust
/// # #[macro_use] extern crate logwise;
/// // At crate root (lib.rs or main.rs)
/// declare_logging_domain!();
/// ```
///
/// With this pattern, you should declare a `logwise_internal` feature in your
/// `Cargo.toml`:
///
/// ```toml
/// [features]
/// logwise_internal = []
/// ```
///
/// Then enable internal logging by running:
/// ```bash
/// cargo build --features logwise_internal
/// ```
///
/// ## 2. Explicit Invocation
///
/// You can explicitly control internal logging by passing a boolean value:
///
/// ```rust
/// # #[macro_use] extern crate logwise;
/// // Always enable internal logging for this crate
/// declare_logging_domain!(true);
/// ```
///
/// Or conditionally based on other criteria:
///
/// ```rust
/// # #[macro_use] extern crate logwise;
/// // Enable based on custom logic or configuration
/// declare_logging_domain!(cfg!(debug_assertions));
/// ```
///
/// # Important Notes
///
/// - **Required for debuginternal macros**: Without this macro at the crate root,
///   attempting to use `debuginternal_sync!` or `debuginternal_async!` will result
///   in a compile error about `__CALL_LOGWISE_DECLARE_LOGGING_DOMAIN` not being found.
///
/// - **Crate-level scope**: This affects only the current crate. Each crate using
///   logwise must declare its own logging domain.
///
/// - **Performance**: When internal logging is disabled, `debuginternal` macros
///   have minimal runtime overhead as they check the domain state early.
///
/// # Examples
///
/// ## Basic Setup with Feature Flag
///
/// `Cargo.toml`:
/// ```toml
/// [dependencies]
/// logwise = "0.1"
///
/// [features]
/// logwise_internal = []
/// ```
///
/// `src/lib.rs`:
/// ```rust
/// # #[macro_use] extern crate logwise;
/// declare_logging_domain!();
/// # fn main() { }
///
/// pub fn my_function() {
///     // This will only log when built with --features logwise_internal
///     logwise::debuginternal_sync!("Debug output: {value}", value=42);
/// }
/// ```
///
/// ## Always-Enabled Internal Logging
///
/// ```rust
/// # #[macro_use] extern crate logwise;
/// // Enable internal logging for all builds of this crate
/// declare_logging_domain!(true);
///
/// fn main() {
///     // This will always log in debug builds
///     logwise::debuginternal_sync!("Application starting");
/// }
/// ```
///
/// ## Conditional Based on Environment
///
/// ```rust
/// # #[macro_use] extern crate logwise;
/// // Enable internal logging based on an environment variable
/// declare_logging_domain!(option_env!("ENABLE_INTERNAL_LOGS").is_some());
/// ```
#[macro_export]
macro_rules! declare_logging_domain {
    () => {
        #[doc(hidden)]
        pub(crate) static __CALL_LOGWISE_DECLARE_LOGGING_DOMAIN: $crate::LoggingDomain =
            $crate::LoggingDomain::new($crate::const_str_eq(
                env!("CARGO_CRATE_NAME"),
                module_path!(),
            ));
    };
    ($enabled:expr) => {
        #[doc(hidden)]
        pub static __CALL_LOGWISE_DECLARE_LOGGING_DOMAIN: $crate::LoggingDomain =
            $crate::LoggingDomain::new($enabled);
    };
}

/// Returns whether logging is enabled for a given [`Level`](crate::Level).
///
/// This macro centralizes the enablement logic used by the logging macros so the
/// same checks are available both inside and outside of the macros themselves.
#[macro_export]
#[allow(clippy::crate_in_macro_def)]
macro_rules! log_enabled {
    ($level:expr) => {
        $crate::log_enabled!($level, crate::__CALL_LOGWISE_DECLARE_LOGGING_DOMAIN)
    };
    ($level:expr, $domain:expr) => {{
        #[allow(unreachable_patterns)]
        match $level {
            $crate::Level::Trace => {
                #[cfg(debug_assertions)]
                {
                    $crate::context::Context::currently_tracing()
                }
                #[cfg(not(debug_assertions))]
                {
                    false
                }
            }
            $crate::Level::DebugInternal => {
                #[cfg(debug_assertions)]
                {
                    ($domain.is_internal()) || $crate::context::Context::currently_tracing()
                }
                #[cfg(not(debug_assertions))]
                {
                    false
                }
            }
            $crate::Level::Info => {
                #[cfg(debug_assertions)]
                {
                    true
                }
                #[cfg(not(debug_assertions))]
                {
                    false
                }
            }
            $crate::Level::PerfWarn
            | $crate::Level::Warning
            | $crate::Level::Error
            | $crate::Level::Panic
            | $crate::Level::Analytics
            | $crate::Level::Mandatory
            | $crate::Level::Profile => true,
            _ => true,
        }
    }};
}

/// Formatter for writing log messages with full private information.
///
/// This formatter is used by the procedural macros to write both literal strings
/// and formatted values to a log record. It ensures that all values are logged
/// with their complete information (including private data) by calling
/// [`Loggable::log_all`](crate::privacy::Loggable::log_all).
///
/// # Example
///
/// This is typically used internally by the procedural macros:
///
/// ```rust
/// # use logwise::{LogRecord, Level};
/// # use logwise::hidden::PrivateFormatter;
/// # use logwise::privacy::Loggable;
/// let mut record = LogRecord::new(Level::Info);
/// let mut formatter = PrivateFormatter::new(&mut record);
/// formatter.write_literal("Count: ");
/// formatter.write_val(42u8);
/// ```
pub struct PrivateFormatter<'a> {
    record: &'a mut LogRecord,
}

impl<'a> PrivateFormatter<'a> {
    /// Creates a new `PrivateFormatter` for the given log record.
    ///
    /// # Arguments
    ///
    /// * `record` - The log record to write to
    #[inline]
    pub fn new(record: &'a mut LogRecord) -> Self {
        Self { record }
    }
    /// Writes a literal string to the log record.
    ///
    /// This is used for the static parts of log messages that don't contain
    /// any variable data.
    ///
    /// # Arguments
    ///
    /// * `s` - The literal string to write
    #[inline]
    pub fn write_literal(&mut self, s: &str) {
        self.record.log(s);
    }
    /// Writes a loggable value to the log record.
    ///
    /// This method calls [`Loggable::log_all`](crate::privacy::Loggable::log_all)
    /// to ensure the value is logged with all its information, including any
    /// private data. This is appropriate for local logging but not for remote
    /// telemetry.
    ///
    /// # Arguments
    ///
    /// * `s` - The value to log (must implement [`Loggable`](crate::privacy::Loggable))
    #[inline]
    pub fn write_val<Val: Loggable>(&mut self, s: Val) {
        s.log_all(self.record);
    }
}

#[cfg(test)]
mod tests {
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_logging_domain_trait_implementations() {
        use crate::LoggingDomain;

        // Test Debug implementation
        let domain_enabled = LoggingDomain::new(true);
        let domain_disabled = LoggingDomain::new(false);
        let debug_enabled = format!("{:?}", domain_enabled);
        let debug_disabled = format!("{:?}", domain_disabled);
        assert!(debug_enabled.contains("LoggingDomain"));
        assert!(debug_disabled.contains("LoggingDomain"));

        // Test Display implementation
        let display_enabled = format!("{}", domain_enabled);
        let display_disabled = format!("{}", domain_disabled);
        assert_eq!(display_enabled, "LoggingDomain(internal=true)");
        assert_eq!(display_disabled, "LoggingDomain(internal=false)");

        // Test Default implementation
        let default_domain = LoggingDomain::default();
        assert!(!default_domain.is_internal());

        // Test From<bool> implementation
        let domain_from_true: LoggingDomain = true.into();
        let domain_from_false: LoggingDomain = false.into();
        assert!(domain_from_true.is_internal());
        assert!(!domain_from_false.is_internal());
    }
}
