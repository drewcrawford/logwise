// SPDX-License-Identifier: MIT OR Apache-2.0

//! Low-level formatting macro implementation.
//!
//! This module provides the `lformat!` macro which is used internally by logging macros
//! to transform format strings with key-value pairs into sequences of formatter calls.

use proc_macro::{TokenStream, TokenTree};
use std::collections::VecDeque;

use crate::parser::lformat_impl;

/// Low-level macro for generating formatter calls from format strings.
///
/// This macro is used internally by the logging macros to transform format strings
/// with key-value pairs into sequences of `write_literal()` and `write_val()` calls
/// on a formatter object. It's exposed publicly for advanced use cases but most users
/// should use the higher-level logging macros instead.
///
/// # Syntax
/// ```
/// # struct Formatter;
/// # impl Formatter {
/// #    fn write_literal(&self, s: &str) {}
/// #    fn write_val(&self, s: u8) {}
/// # }
/// # let format_ident = Formatter;
/// # use logwise_proc::lformat;
/// lformat!(format_ident, "format string with {key1} {key2}", key1=2, key2=3);
/// ```
///
/// # Arguments
/// * `formatter_ident` - The name of the formatter variable to call methods on
/// * `format_string` - A string literal with `{key}` placeholders
/// * `key=value` pairs - Values to substitute for placeholders in the format string
///
/// # Generated Code
/// The macro generates a sequence of method calls on the formatter:
/// - `formatter.write_literal("text")` for literal text portions
/// - `formatter.write_val(expression)` for placeholder values
///
/// # Examples
///
/// Basic usage with a mock formatter:
/// ```
/// # struct Logger;
/// # impl Logger {
/// #   fn write_literal(&self, s: &str) {}
/// #   fn write_val(&self, s: u8) {}
/// # }
/// # let logger = Logger;
/// use logwise_proc::lformat;
/// lformat!(logger,"Hello, {world}!", world=23);
/// ```
///
/// This expands to approximately:
/// ```ignore
/// # // ignore because: This shows macro expansion output, not actual runnable code
/// logger.write_literal("Hello, ");
/// logger.write_val(23);
/// logger.write_literal("!");
/// ```
///
/// Complex expressions as values:
/// ```
/// # struct Logger;
/// # impl Logger {
/// #   fn write_literal(&self, s: &str) {}
/// #   fn write_val<A>(&self, s: A) {}
/// # }
/// # let logger = Logger;
/// # use logwise_proc::lformat;
/// // This works with any expression
/// lformat!(logger, "User {a} has {b} items",
///          a = "hi",
///          b = 3);
/// ```
///
/// # Error Cases
///
/// Missing formatter identifier:
/// ```compile_fail
/// use logwise_proc::lformat;
/// lformat!(23);
/// ```
///
/// Missing key in format string:
/// ```compile_fail
/// # struct Logger;
/// # impl Logger {
/// #   fn write_literal(&self, s: &str) {}
/// #   fn write_val(&self, s: u8) {}
/// # }
/// # let logger = Logger;
/// use logwise_proc::lformat;
/// lformat!(logger, "Hello {missing}!", provided=123);
/// ```
pub fn lformat_macro(input: TokenStream) -> TokenStream {
    let mut collect: VecDeque<_> = input.into_iter().collect();

    //get logger ident
    let logger_ident = match collect.pop_front() {
        Some(TokenTree::Ident(i)) => i,
        _ => {
            return r#"compile_error!("lformat!() must be called with a logger ident")"#
                .parse()
                .unwrap();
        }
    };
    //eat comma
    match collect.pop_front() {
        Some(TokenTree::Punct(p)) => {
            if p.as_char() != ',' {
                return r#"compile_error!("Expected ','")"#.parse().unwrap();
            }
        }
        _ => {
            return r#"compile_error!("Expected ','")"#.parse().unwrap();
        }
    }

    let o = lformat_impl(&mut collect, logger_ident.to_string());
    o.output
}
