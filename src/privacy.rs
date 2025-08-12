// SPDX-License-Identifier: MIT OR Apache-2.0

//! Privacy-aware logging system for logwise.
//!
//! This module provides the foundation for logwise's dual-representation logging system,
//! which allows the same log statement to produce different output depending on privacy
//! requirements. This is crucial for maintaining user privacy while still enabling
//! effective debugging and monitoring.
//!
//! # Core Concepts
//!
//! The privacy system revolves around the [`Loggable`] trait, which defines two methods
//! for logging:
//!
//! - `log_redacting_private_info()`: Produces safe output for remote servers
//! - `log_all()`: Produces complete output for local/private logging
//!
//! # Privacy Philosophy
//!
//! The library takes an opinionated stance on what data types are considered potentially
//! private:
//!
//! - **Small integers** (`u8`, `i8`): Considered safe (often enum discriminants, counts)
//! - **Larger numeric types**: Considered potentially private (could be IDs, timestamps)
//! - **Strings**: Always considered potentially private
//! - **Booleans and chars**: Considered safe
//!
//! # Usage Patterns
//!
//! ## Basic Usage
//!
//! ```
//! # use logwise::privacy::Loggable;
//! # use logwise::LogRecord;
//! # use logwise::Level;
//! # let mut record = LogRecord::new(Level::Info);
//! // Numeric types have built-in privacy awareness
//! let count: u8 = 5;  // Safe to log
//! let user_id: u64 = 12345;  // Will be redacted
//!
//! count.log_redacting_private_info(&mut record);  // Logs "5"
//! user_id.log_redacting_private_info(&mut record); // Logs "<u64>"
//! ```
//!
//! ## Privacy Wrappers
//!
//! When you need explicit control over privacy:
//!
//! ```
//! # use logwise::privacy::{LogIt, IPromiseItsNotPrivate};
//! // Force private logging (never sent to remote servers)
//! let sensitive = LogIt("credit_card_number");
//!
//! // Promise data is safe for remote logging
//! let safe = IPromiseItsNotPrivate("public_event_name");
//! ```
//!
//! ## Integration with Logging Macros
//!
//! ```
//! # let user_id = "user123";
//! # let request_id = 42u32;
//! // The macros automatically use the Loggable trait
//! logwise::info_sync!("Processing request {id}", id=request_id);
//!
//! // Use wrappers for explicit control
//! logwise::info_sync!("User {user} logged in",
//!                     user=logwise::privacy::LogIt(user_id));
//! ```

use crate::log_record::LogRecord;
use std::fmt::Debug;

/// Trait for types that can be logged with privacy awareness.
///
/// This trait enables dual-representation logging, where the same value can be
/// logged differently depending on whether it's being sent to a remote server
/// (where privacy matters) or being logged locally (where full details are useful).
///
/// # Implementation Guidelines
///
/// When implementing this trait:
/// - Use `#[inline]` on both methods for performance
/// - `log_redacting_private_info` should redact any potentially sensitive data
/// - `log_all` should provide complete information for debugging
/// - Consider whether your type contains user data, IDs, or other sensitive information
///
/// # Example Implementation
///
/// ```
/// use logwise::privacy::Loggable;
/// use logwise::LogRecord;
///
/// struct UserId(u64);
///
/// impl Loggable for UserId {
///     #[inline]
///     fn log_redacting_private_info(&self, record: &mut LogRecord) {
///         // Redact the actual ID for remote logging
///         record.log("<UserId>");
///     }
///     
///     #[inline]
///     fn log_all(&self, record: &mut LogRecord) {
///         // Log the full ID for local debugging
///         record.log_owned(format!("UserId({})", self.0));
///     }
/// }
/// ```
pub trait Loggable {
    /// Logs the object with privacy redaction for remote servers.
    ///
    /// This method should produce a representation that is safe to send to
    /// remote logging servers. Any potentially sensitive information should
    /// be redacted or replaced with type indicators.
    ///
    /// # Performance Note
    ///
    /// Implementations should use `#[inline]` for optimal performance.
    fn log_redacting_private_info(&self, record: &mut LogRecord);

    /// Logs the complete object for private/local logging.
    ///
    /// This method should produce a complete representation suitable for
    /// local debugging and troubleshooting. All information should be included.
    ///
    /// # Performance Note
    ///
    /// Implementations should use `#[inline]` for optimal performance.
    fn log_all(&self, record: &mut LogRecord);
}

/// Implementation for `u8` - considered safe to log.
///
/// Small integers like `u8` are typically used for counts, enum discriminants,
/// or other non-sensitive values, so they are logged in full even when redacting.
///
/// # Example
///
/// ```
/// # use logwise::privacy::Loggable;
/// # use logwise::LogRecord;
/// # use logwise::Level;
/// # let mut record = LogRecord::new(Level::Info);
/// let count: u8 = 42;
/// count.log_redacting_private_info(&mut record); // Logs "42"
/// count.log_all(&mut record);                    // Also logs "42"
/// ```
impl Loggable for u8 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        self.log_redacting_private_info(record);
    }
}

/// Implementation for `u16` - considered potentially private.
///
/// Values like `u16` might contain IDs, ports, or other sensitive information,
/// so they are redacted when logging to remote servers.
///
/// # Example
///
/// ```
/// # use logwise::privacy::Loggable;
/// # use logwise::LogRecord;
/// # use logwise::Level;
/// # let mut record = LogRecord::new(Level::Info);
/// let port: u16 = 8080;
/// port.log_redacting_private_info(&mut record); // Logs "<u16>"
/// port.log_all(&mut record);                    // Logs "8080"
/// ```
impl Loggable for u16 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<u16>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/// Implementation for `u32` - considered potentially private.
///
/// Values like `u32` might contain IDs, timestamps, or other sensitive information.
impl Loggable for u32 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<u32>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/// Implementation for `u64` - considered potentially private.
///
/// Large integers often represent IDs, timestamps, or other sensitive values.
impl Loggable for u64 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<u64>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/// Implementation for `usize` - considered potentially private.
///
/// Size values might reveal information about data structures or memory usage.
impl Loggable for usize {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<usize>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/// Implementation for `u128` - considered potentially private.
///
/// Very large integers are often UUIDs or other unique identifiers.
impl Loggable for u128 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<u128>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/// Implementation for `i8` - considered safe to log.
///
/// Small signed integers are typically used for small counts or enum values.
impl Loggable for i8 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        self.log_redacting_private_info(record);
    }
}

/// Implementation for `i16` - considered potentially private.
impl Loggable for i16 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<i16>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/// Implementation for `i32` - considered potentially private.
impl Loggable for i32 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<i32>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/// Implementation for `i64` - considered potentially private.
impl Loggable for i64 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<i64>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/// Implementation for `i128` - considered potentially private.
impl Loggable for i128 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<i128>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/// Implementation for `f32` - considered potentially private.
///
/// Floating point values might contain coordinates, measurements, or calculations.
impl Loggable for f32 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<f32>");
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/// Implementation for `f64` - considered potentially private.
///
/// Double precision floats often contain precise measurements or coordinates.
impl Loggable for f64 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<f64>");
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/// Implementation for `bool` - considered safe to log.
///
/// Boolean values are simple flags and are always safe to log.
impl Loggable for bool {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/// Implementation for `char` - considered safe to log.
///
/// Individual characters are typically safe to log.
impl Loggable for char {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/// Implementation for `String` - considered potentially private.
///
/// Strings often contain user data, names, or other sensitive information,
/// so they are always redacted when logging to remote servers.
///
/// # Example
///
/// ```
/// # use logwise::privacy::Loggable;
/// # use logwise::LogRecord;
/// # use logwise::Level;
/// # let mut record = LogRecord::new(Level::Info);
/// let username = String::from("alice");
/// username.log_redacting_private_info(&mut record); // Logs "<String>"
/// username.log_all(&mut record);                    // Logs "alice"
/// ```
impl Loggable for String {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<String>");
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(self.clone());
    }
}

/// Implementation for `&str` - considered potentially private.
///
/// String slices often contain user data or other sensitive information,
/// so they are always redacted when logging to remote servers.
///
/// # Example
///
/// ```
/// # use logwise::privacy::Loggable;
/// # use logwise::LogRecord;
/// # use logwise::Level;
/// # let mut record = LogRecord::new(Level::Info);
/// let message = "sensitive data";
/// message.log_redacting_private_info(&mut record); // Logs "<&str>"
/// message.log_all(&mut record);                    // Logs "sensitive data"
/// ```
impl Loggable for &str {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<&str>");
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(self.to_string());
    }
}

/// Implementation for slices - privacy depends on the element type.
///
/// Slices respect the privacy settings of their contained type.
///
/// # Example
///
/// ```
/// # use logwise::privacy::Loggable;
/// # use logwise::LogRecord;
/// # use logwise::Level;
/// # let mut record = LogRecord::new(Level::Info);
/// let safe_values: &[u8] = &[1, 2, 3];
/// let private_values: &[u32] = &[100, 200, 300];
///
/// safe_values.log_redacting_private_info(&mut record);    // Logs "<[1, 2, 3, ]>"
/// private_values.log_redacting_private_info(&mut record); // Logs "<[<u32>, <u32>, <u32>, ]>"
/// ```
impl<T: Loggable> Loggable for &[T] {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<[");
        for item in self.iter() {
            item.log_redacting_private_info(record);
            record.log(", ");
        }
        record.log("]>");
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log("<[");
        for item in self.iter() {
            item.log_all(record);
            record.log(", ");
        }
        record.log("]>");
    }
}

/// Implementation for `Option<T>` - privacy depends on the inner type.
///
/// Options respect the privacy settings of their contained type.
///
/// # Example
///
/// ```
/// # use logwise::privacy::Loggable;
/// # use logwise::LogRecord;
/// # use logwise::Level;
/// # let mut record = LogRecord::new(Level::Info);
/// let maybe_id: Option<u64> = Some(12345);
/// let nothing: Option<String> = None;
///
/// maybe_id.log_redacting_private_info(&mut record); // Logs "Some(<u64>)"
/// nothing.log_redacting_private_info(&mut record);  // Logs "None"
/// ```
impl<T: Loggable> Loggable for Option<T> {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        match self {
            Some(t) => {
                record.log("Some(");
                t.log_redacting_private_info(record);
                record.log(")");
            }
            None => record.log("None"),
        }
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        match self {
            Some(t) => {
                record.log("Some(");
                t.log_all(record);
                record.log(")");
            }
            None => record.log("None"),
        }
    }
}

/// A wrapper that ensures data is never sent to remote servers.
///
/// `LogIt` is used when you have data that should be logged locally for debugging
/// but must never be sent to remote logging servers. The wrapped value will be
/// logged in full detail locally but replaced with `<LogIt>` for remote logging.
///
/// # When to Use
///
/// Use `LogIt` when:
/// - The data contains sensitive information (passwords, tokens, PII)
/// - You need the data for local debugging but it's not safe for remote storage
/// - The type doesn't implement `Loggable` but implements `Debug`
///
/// # Example
///
/// ```
/// use logwise::privacy::LogIt;
///
/// #[derive(Debug)]
/// struct CreditCard {
///     number: String,
///     cvv: String,
/// }
///
/// let card = CreditCard {
///     number: "1234-5678-9012-3456".to_string(),
///     cvv: "123".to_string(),
/// };
///
/// // This will log the full credit card locally but "<LogIt>" remotely
/// logwise::info_sync!("Processing payment with {card}",
///                     card=LogIt(card));
/// ```
///
/// # Integration with Complex Types
///
/// ```
///
/// use logwise::privacy::LogIt;
/// use std::collections::HashMap;
///
/// let mut sensitive_data = HashMap::new();
/// sensitive_data.insert("ssn", "123-45-6789");
/// sensitive_data.insert("dob", "1990-01-01");
///
/// // Wrap the entire HashMap to protect all contents
/// logwise::info_sync!("User data: {data}",
///                              data=LogIt(&sensitive_data));
/// ```
pub struct LogIt<T>(pub T);

impl<T: Debug> Loggable for LogIt<T> {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<LogIt>");
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{:?}", self.0));
    }
}

/// A wrapper that explicitly marks data as safe for remote logging.
///
/// `IPromiseItsNotPrivate` is used when you have data that would normally be
/// redacted (like strings or large integers) but you know it's safe to log
/// to remote servers. This is a promise to the logging system that the wrapped
/// value contains no sensitive information.
///
/// # When to Use
///
/// Use `IPromiseItsNotPrivate` when:
/// - You have public identifiers that happen to be strings
/// - You have non-sensitive numeric IDs stored in normally-redacted types
/// - You need to log metadata that the privacy system would normally redact
///
/// # Safety Contract
///
/// By using this wrapper, you are making an explicit promise that the data
/// contains no private or sensitive information. Use with caution!
///
/// # Example
///
/// ```
/// use logwise::privacy::IPromiseItsNotPrivate;
///
/// // These would normally be redacted as strings
/// let event_type = "user_login";
/// let server_name = "api-server-01";
///
/// // But we know these are safe metadata, not user data
/// logwise::info_sync!("Event {event} on {server}",
///                     event=IPromiseItsNotPrivate(event_type),
///                     server=IPromiseItsNotPrivate(server_name));
/// ```
///
/// # Example with IDs
///
/// ```
/// use logwise::privacy::IPromiseItsNotPrivate;
///
/// // This would normally be redacted as a u64
/// let public_product_id: u64 = 789456;
///
/// // But we know product IDs are public information
/// logwise::warn_sync!("Product {id} is out of stock",
///                     id=IPromiseItsNotPrivate(public_product_id));
/// ```
///
/// # Contrast with Private Data
///
/// ```
/// use logwise::privacy::{IPromiseItsNotPrivate, LogIt};
///
/// let public_error_code = "ERR_404";  // Safe to log
/// let user_email = "user@example.com"; // Private data
///
/// // Correct usage:
/// logwise::error_sync!("Error {code} for user {user}",
///                      code=IPromiseItsNotPrivate(public_error_code),
///                      user=LogIt(user_email));
///
/// // WRONG - Don't do this! User email is private:
/// // code=IPromiseItsNotPrivate(user_email)  // DON'T DO THIS!
/// ```
pub struct IPromiseItsNotPrivate<T>(pub T);

impl<T: Debug> Loggable for IPromiseItsNotPrivate<T> {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        Self::log_all(self, record);
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{:?}", self.0));
    }
}
