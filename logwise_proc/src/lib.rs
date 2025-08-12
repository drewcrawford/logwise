//SPDX-License-Identifier: MIT OR Apache-2.0

//! # Logwise Procedural Macros
//!
//! This crate provides procedural macros for the logwise logging library, generating efficient
//! structured logging code at compile time. The macros transform format strings with key-value
//! pairs into optimized logging calls that use the `PrivateFormatter` system.
//!
//! ## Architecture
//!
//! Each logging macro follows a consistent three-phase pattern:
//! 1. **Pre-phase**: Creates a `LogRecord` using `*_pre()` functions with location metadata
//! 2. **Format phase**: Uses `PrivateFormatter` to write structured data via `lformat_impl`
//! 3. **Post-phase**: Completes logging using `*_post()` functions (sync or async variants)
//!
//! ## Log Levels and Build Configuration
//!
//! The macros respect logwise's opinionated logging levels:
//! - `trace_*`: Debug builds only, per-thread activation via `Context::currently_tracing()`
//! - `debuginternal_*`: Debug builds only, requires `declare_logging_domain!()` at crate root
//! - `info_*`: Debug builds only, for supporting downstream crates
//! - `warn_*`, `error_*`, `perfwarn_*`: Available in all builds
//!
//! ## Usage Example
//!
//! ```rust
//! use logwise_proc::*;
//!
//! // This macro call:
//! // logwise::info_sync!("User {name} has {count} items", name="alice", count=42);
//!
//! // Expands to approximately:
//! // {
//! //     let mut record = logwise::hidden::info_sync_pre(file!(), line!(), column!());
//! //     let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
//! //     formatter.write_literal("User ");
//! //     formatter.write_val("alice");
//! //     formatter.write_literal(" has ");
//! //     formatter.write_val(42);
//! //     formatter.write_literal(" items");
//! //     logwise::hidden::info_sync_post(record);
//! // }
//! ```
//!
//! ## Key-Value Parsing
//!
//! Format strings support embedded key-value pairs:
//! - Keys are extracted from `{key}` placeholders in the format string
//! - Values are provided as `key=value` parameters after the format string
//! - The parser handles complex Rust expressions as values, including method calls and literals
//!
//! ## Privacy Integration
//!
//! These macros integrate with logwise's privacy system via the `Loggable` trait.
//! Values are processed through `formatter.write_val()` which respects privacy constraints.

use proc_macro::{TokenStream, TokenTree};
use std::collections::{HashMap, VecDeque};

/// Parses a key from the token stream, consuming tokens until '=' is encountered.
///
/// This function extracts the left-hand side of key-value pairs in macro arguments.
/// It continues consuming tokens (identifiers, literals, groups) until it finds an
/// equals sign, which signals the start of the value portion.
///
/// # Arguments
/// * `input` - Mutable reference to the token stream being parsed
///
/// # Returns
/// * `Some(String)` - The key if '=' was found
/// * `Some("".to_string())` - Empty string if a non-'=' punctuation was found
/// * `None` - If the stream was exhausted without finding '='
///
/// # Examples
/// ```ignore
/// // For input: `name = "value"`
/// // Returns: Some("name".to_string())
/// // Consumes: `name`, stops at `=`
/// ```
fn parse_key(input: &mut VecDeque<TokenTree>) -> Option<String> {
    //basically we go until we get a =.
    let mut key = String::new();
    loop {
        match input.pop_front() {
            Some(TokenTree::Punct(p)) => {
                if p.as_char() == '=' {
                    return Some(key);
                }
                return Some("".to_string());
            }
            Some(TokenTree::Ident(i)) => {
                key.push_str(&i.to_string());
            }
            Some(TokenTree::Literal(l)) => {
                key.push_str(&l.to_string());
            }
            Some(TokenTree::Group(g)) => {
                key.push_str(&g.to_string());
            }
            None => {
                return None;
            }
        }
    }
}

/// Parses a value from the token stream, consuming tokens until ',' or end of stream.
///
/// This function extracts the right-hand side of key-value pairs in macro arguments.
/// It reconstructs the original Rust expression as a string, handling all token types
/// including complex expressions with method calls, operators, and nested structures.
///
/// # Arguments
/// * `input` - Mutable reference to the token stream being parsed
///
/// # Returns
/// * `String` - The complete value expression as a string
///
/// # Examples
/// ```ignore
/// // For input: `user.name.clone(), next_param = value`
/// // Returns: "user.name.clone()".to_string()
/// // Consumes everything until the comma
/// ```
fn parse_value(input: &mut VecDeque<TokenTree>) -> String {
    //basically we go until we get a , or end.
    let mut value = String::new();
    loop {
        match input.pop_front() {
            Some(TokenTree::Punct(p)) => {
                if p.as_char() == ',' {
                    return value;
                }
                value.push_str(&p.to_string());
            }
            Some(TokenTree::Ident(i)) => {
                value.push_str(&i.to_string());
            }
            Some(TokenTree::Literal(l)) => {
                value.push_str(&l.to_string());
            }
            Some(TokenTree::Group(g)) => {
                value.push_str(&g.to_string());
            }
            None => {
                return value;
            }
        }
    }
}

/// Builds a HashMap of key-value pairs from the remaining token stream.
///
/// This function processes the parameter list portion of logging macros, extracting
/// all key-value pairs that follow the format string. It expects the first token
/// to be a comma separator, then processes alternating key=value pairs.
///
/// # Arguments
/// * `input` - Mutable reference to the token stream containing key-value pairs
///
/// # Returns
/// * `Ok(HashMap<String, String>)` - Successfully parsed key-value pairs
/// * `Err(TokenStream)` - Compile error if the format is invalid
///
/// # Expected Input Format
/// ```ignore
/// // After format string: , key1=value1, key2=value2, key3=complex_expr()
/// ```
///
/// # Examples
/// ```ignore
/// // Input: `, name="alice", count=42`
/// // Output: HashMap { "name" => "\"alice\"", "count" => "42" }
/// ```
fn build_kvs(input: &mut VecDeque<TokenTree>) -> Result<HashMap<String, String>, TokenStream> {
    let mut kvs = HashMap::new();
    //first extract the comma.
    if input.is_empty() {
        return Ok(kvs);
    }
    match input.pop_front() {
        Some(TokenTree::Punct(p)) => {
            if p.as_char() != ',' {
                return Err(r#"compile_error!("Expected ','")"#.parse().unwrap());
            }
        }
        _ => {
            return Err(r#"compile_error!("Expected ','")"#.parse().unwrap());
        }
    }
    loop {
        let key = match parse_key(input) {
            Some(k) => k,
            None => {
                return Ok(kvs);
            }
        };
        let value = parse_value(input);
        kvs.insert(key, value);
    }
}

/// Result of processing a format string through `lformat_impl`.
///
/// This struct contains both the generated logging code and metadata about
/// the processed format string, used by different macro variants.
///
/// # Fields
/// * `output` - The generated `TokenStream` containing `formatter.write_*()` calls
/// * `name` - The original format string (used for performance interval naming)
///
/// # Usage
/// The `output` field contains code like:
/// ```ignore
/// formatter.write_literal("Hello, ");
/// formatter.write_val(username);
/// formatter.write_literal("!");
/// ```
struct LFormatResult {
    output: TokenStream,
    name: String,
}

/// Core implementation for format string processing and code generation.
///
/// This function transforms a format string with embedded `{key}` placeholders into
/// a sequence of `formatter.write_literal()` and `formatter.write_val()` calls.
/// It handles escaping (via `{{` and `}}`), validates key-value pairs, and generates
/// efficient logging code.
///
/// # Arguments
/// * `collect` - Mutable token stream containing format string and key-value pairs
/// * `logger` - Name of the formatter variable to generate calls for
///
/// # Returns
/// * `LFormatResult` - Generated code and original format string
///
/// # Processing Logic
/// 1. Extracts and validates the format string literal
/// 2. Parses key-value pairs from remaining tokens
/// 3. Processes format string character by character:
///    - Literal text becomes `formatter.write_literal("text")`
///    - `{key}` becomes `formatter.write_val(value)` using the key-value map
///    - `{{` and `}}` are treated as escaped braces
/// 4. Validates that all keys in format string have corresponding values
///
/// # Error Conditions
/// - Non-string-literal format string
/// - Missing key-value pairs
/// - Malformed key-value syntax
/// - Unclosed braces in format string
///
/// # Examples
/// ```ignore
/// // Input: "Hello {name}!", name="world"
/// // Generates:
/// // formatter.write_literal("Hello ");
/// // formatter.write_val("world");
/// // formatter.write_literal("!");
/// ```
fn lformat_impl(collect: &mut VecDeque<TokenTree>, logger: String) -> LFormatResult {
    let some_input = match collect.remove(0) {
        Some(i) => i,
        None => {
            return LFormatResult {
                output: r#"compile_error!("lformat!() must be called with a string literal")"#
                    .parse()
                    .unwrap(),
                name: "".to_string(),
            }
        }
    };
    let format_string = match some_input {
        TokenTree::Literal(l) => {
            let out = l.to_string();
            if !out.starts_with('"') || !out.ends_with('"') {
                return LFormatResult {
                    output: r#"compile_error!("lformat!() must be called with a string literal")"#
                        .parse()
                        .unwrap(),
                    name: "".to_string(),
                };
            }
            out[1..out.len() - 1].to_string()
        }
        _ => {
            return LFormatResult {
                output: r#"compile_error!("lformat!() must be called with a string literal")"#
                    .parse()
                    .unwrap(),
                name: "".to_string(),
            };
        }
    };

    //parse kv section
    let k = match build_kvs(collect) {
        Ok(kvs) => kvs,
        Err(e) => {
            return LFormatResult {
                output: e,
                name: "".to_string(),
            };
        }
    };
    //parse format string
    //holds the part of the string literal until the next {
    let mut source = String::new();
    enum Mode {
        Literal(String),
        Key(String),
    }
    let mut mode = Mode::Literal(String::new());

    for (c, char) in format_string.chars().enumerate() {
        match mode {
            Mode::Literal(mut literal) => {
                if char == '{' {
                    //peek to see if we're escaping
                    if format_string.chars().nth(c + 1) == Some('{') {
                        literal.push(char);
                        mode = Mode::Literal(literal);
                    } else if !literal.is_empty() {
                        //reference logger ident
                        source.push_str(&logger);
                        source.push_str(".write_literal(\"");
                        source.push_str(&literal);
                        source.push_str("\");\n");
                        mode = Mode::Key(String::new());
                    } else {
                        mode = Mode::Key(String::new());
                    }
                } else {
                    literal.push(char);
                    mode = Mode::Literal(literal);
                }
            }
            Mode::Key(mut key) => {
                if char == '}' {
                    //write out the key
                    source.push_str(&logger);
                    source.push_str(".write_val(");
                    let value = match k.get(&key) {
                        Some(l) => l.to_string(),
                        None => {
                            return LFormatResult {
                                output: format!(r#"compile_error!("Key {} not found")"#, key)
                                    .parse()
                                    .unwrap(),
                                name: "".to_string(),
                            };
                        }
                    };
                    source.push_str(&value);
                    source.push_str(");\n");
                    mode = Mode::Literal(String::new());
                } else {
                    key.push(char);
                    mode = Mode::Key(key);
                }
            }
        }
    }
    //check end situation
    match mode {
        Mode::Literal(l) => {
            if !l.is_empty() {
                source.push_str(&logger.to_string());
                source.push_str(".write_literal(\"");
                source.push_str(&l);
                source.push_str("\");\n");
            }
        }
        Mode::Key(_) => {
            return LFormatResult {
                output: r#"compile_error!("Expected '}'")"#.parse().unwrap(),
                name: "".to_string(),
            };
        }
    }
    return LFormatResult {
        output: source.parse().unwrap(),
        name: format_string,
    };
}

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
#[proc_macro]
pub fn lformat(input: TokenStream) -> TokenStream {
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

/// Generates synchronous trace-level logging code (debug builds only).
///
/// This macro creates trace-level log entries that are only active in debug builds
/// and when `Context::currently_tracing()` returns true. Trace logging is designed
/// for detailed debugging information that can be activated per-thread.
///
/// # Syntax
/// ```
/// logwise::trace_sync!("format string {key}", key="value");
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Active when `Context::currently_tracing()` is true
/// - **Release builds**: Completely compiled out (no runtime cost)
///
/// # Generated Code Pattern
/// ```ignore
/// #[cfg(debug_assertions)]
/// {
///     if logwise::context::Context::currently_tracing() {
///         let mut record = logwise::hidden::trace_sync_pre(file!(), line!(), column!());
///         let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///         // ... formatter calls ...
///         logwise::hidden::trace_sync_post(record);
///     }
/// }
/// ```
///
/// # Examples
/// ```
/// // Basic trace logging
/// logwise::trace_sync!("Entering function with param {value}", value=42);
///
/// // With complex expressions
/// logwise::trace_sync!("User {name} processing", name="john");
///
/// // With privacy-aware values
/// let sensitive_data = "secret job";
/// logwise::trace_sync!("Processing {data}", data=logwise::privacy::LogIt(sensitive_data));
/// ```
#[proc_macro]
pub fn trace_sync(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)]
        {{
            if logwise::context::Context::currently_tracing() {{
                let mut record = logwise::hidden::trace_sync_pre(file!(),line!(),column!());

                let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);

                {LFORMAT_EXPAND}
                logwise::hidden::trace_sync_post(record);
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}

/// Generates asynchronous trace-level logging code (debug builds only).
///
/// This macro creates async trace-level log entries that are only active in debug builds
/// and when `Context::currently_tracing()` returns true. The async variant is suitable
/// for use in async contexts where logging might involve async operations.
///
/// # Syntax
/// ```
/// async fn example() {
///     logwise::trace_async!("format string");
/// }
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Active when `Context::currently_tracing()` is true
/// - **Release builds**: Completely compiled out (no runtime cost)
///
/// # Generated Code Pattern
/// ```ignore
/// #[cfg(debug_assertions)]
/// {
///     if logwise::context::Context::currently_tracing() {
///         let mut record = logwise::hidden::trace_sync_pre(file!(), line!(), column!());
///         let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///         // ... formatter calls ...
///         logwise::hidden::trace_async_post(record).await;
///     }
/// }
/// ```
///
/// # Examples
/// ```
/// // In async function context
/// async fn process_data(data: &[u8]) {
///     logwise::trace_async!("Processing {size} bytes", size=data.len());
///     // ... async work ...
/// }
/// ```
#[proc_macro]
pub fn trace_async(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)]
        {{
            if logwise::context::Context::currently_tracing() {{
                let mut record = logwise::hidden::trace_sync_pre(file!(),line!(),column!());

                let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);

                {LFORMAT_EXPAND}
                logwise::hidden::trace_async_post(record).await;
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}

/// Generates synchronous debug-internal logging code (debug builds only).
///
/// This macro creates debug-internal log entries that are only active in debug builds.
/// It's designed for print-style debugging within the current crate and requires the
/// `declare_logging_domain!()` macro to be used at the crate root.
///
/// # Prerequisites
/// You must declare a logging domain at your crate root:
/// ```
/// logwise::declare_logging_domain!();
/// ```
///
/// # Syntax
/// ```
/// logwise::declare_logging_domain!();
/// fn main() {
/// let key = "example_value";
/// logwise::debuginternal_sync!("format string with {placeholders}", placeholders=key);
/// }
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Active when domain is internal OR `Context::currently_tracing()` is true
/// - **Release builds**: Completely compiled out (no runtime cost)
///
/// # Generated Code Pattern
/// ```ignore
/// #[cfg(debug_assertions)]
/// {
///     let use_declare_logging_domain_macro_at_crate_root = crate::__LOGWISE_DOMAIN.is_internal();
///     if use_declare_logging_domain_macro_at_crate_root || logwise::context::Context::currently_tracing() {
///         let mut record = logwise::hidden::debuginternal_pre(file!(), line!(), column!());
///         let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///         // ... formatter calls ...
///         logwise::hidden::debuginternal_sync_post(record);
///     }
/// }
/// ```
///
/// # Examples
/// ```
/// logwise::declare_logging_domain!();
/// fn main() {
/// // In your code:
/// let current_state = 42;
/// logwise::debuginternal_sync!("Debug: value is {val}", val=current_state);
///
/// // With complex expressions
/// # use std::collections::VecDeque;
/// let mut queue = VecDeque::new();
/// queue.push_front("item");
/// logwise::debuginternal_sync!("Processing {item}", item=logwise::privacy::LogIt(&queue.front().unwrap()));
/// }
/// ```
///
/// # Error Conditions
/// If you forget to use declare_logging_domain!(), you'll get a compile error
/// about __LOGWISE_DOMAIN not being found.
#[proc_macro]
pub fn debuginternal_sync(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)] {{
        let use_declare_logging_domain_macro_at_crate_root = crate::__LOGWISE_DOMAIN.is_internal();
            if use_declare_logging_domain_macro_at_crate_root || logwise::context::Context::currently_tracing() {{
                    let mut record = logwise::hidden::debuginternal_pre(file!(),line!(),column!());
                    let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
                    {LFORMAT_EXPAND}
                    logwise::hidden::debuginternal_sync_post(record);
           }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );
    // todo!("{}", src);
    src.parse().unwrap()
}

/// Generates asynchronous debug-internal logging code (debug builds only).
///
/// This macro creates async debug-internal log entries that are only active in debug builds.
/// Like the sync variant, it requires `declare_logging_domain!()` at the crate root and is
/// designed for print-style debugging in async contexts.
///
/// # Prerequisites
/// You must declare a logging domain at your crate root:
/// ```
/// logwise::declare_logging_domain!();
/// ```
///
/// # Syntax
/// ```
/// logwise::declare_logging_domain!(true);
/// fn main() {
///     async fn ex() {
///         let key = "example_value";
///         logwise::debuginternal_async!("format string with {placeholders}", placeholders=key);
///     }
/// }
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Active when domain is internal OR `Context::currently_tracing()` is true
/// - **Release builds**: Completely compiled out (no runtime cost)
///
/// # Examples
/// ```
/// # fn main() { }
/// # logwise::declare_logging_domain!();
/// // In async functions:
/// async fn process_async() {
///     let task_id = "task_123";
///     logwise::debuginternal_async!("Starting async work with {id}", id=task_id);
///     // ... async operations ...
/// }
/// ```
#[proc_macro]
pub fn debuginternal_async(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)] {{
        let use_declare_logging_domain_macro_at_crate_root = crate::__LOGWISE_DOMAIN.is_internal();
            if use_declare_logging_domain_macro_at_crate_root || logwise::context::Context::currently_tracing() {{
               let mut record = logwise::hidden::debuginternal_pre(file!(),line!(),column!());
                let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);

                {LFORMAT_EXPAND}
                logwise::hidden::debuginternal_async_post(record).await;
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}

/// Generates synchronous info-level logging code (debug builds only).
///
/// This macro creates info-level log entries designed for supporting downstream crates.
/// Info logging is intended for important operational information that helps users
/// understand what the library is doing, but is only active in debug builds.
///
/// # Syntax
/// ```
/// let key = "example_value";
/// logwise::info_sync!("format string with {placeholders}", placeholders=key);
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Completely compiled out (no runtime cost)
///
/// # Generated Code Pattern
/// ```
/// #[cfg(debug_assertions)]
/// {
///     let mut record = logwise::hidden::info_sync_pre(file!(), line!(), column!());
///     let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///     // ... formatter calls ...
///     logwise::hidden::info_sync_post(record);
/// }
/// ```
///
/// # Examples
/// ```
/// // Library operation notifications
/// # struct Pool; impl Pool { fn active_count(&self) -> usize { 42 } }
/// # let pool = Pool;
/// logwise::info_sync!("Connected to database with {connections} active", connections=pool.active_count());
///
/// // User-facing operation status
/// # let items = vec![1, 2, 3];
/// logwise::info_sync!("Processing {count} items", count=items.len());
///
/// // With privacy considerations
/// # let user_id = "user123";
/// logwise::info_sync!("User {id} authenticated", id=logwise::privacy::LogIt(user_id));
/// ```
#[proc_macro]
pub fn info_sync(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)]
        {{
            let mut record = logwise::hidden::info_sync_pre(file!(),line!(),column!());

            let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);

            {LFORMAT_EXPAND}
            logwise::hidden::info_sync_post(record);
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}

/// Generates asynchronous info-level logging code (debug builds only).
///
/// This macro creates async info-level log entries designed for supporting downstream crates
/// in async contexts. Like the sync variant, it's intended for important operational
/// information but only active in debug builds.
///
/// # Syntax
/// ```
/// async fn example() {
///     let key = "example_value";
///     logwise::info_async!("format string with {placeholders}", placeholders=key);
/// }
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Completely compiled out (no runtime cost)
///
/// # Examples
/// ```
/// struct DbConfig { host: String }
///
/// async fn example() {
///     let db_config = DbConfig { host: "localhost".to_string() };
///     logwise::info_async!("Attempting database connection to {host}", host=db_config.host);
///     // ... connection logic ...
///
///     // Progress reporting in async contexts
///     let current = 5;
///     let total_batches = 10;
///     logwise::info_async!("Processed batch {batch} of {total}", batch=current, total=total_batches);
/// }
/// ```
#[proc_macro]
pub fn info_async(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)] {{
            let mut record = logwise::hidden::info_sync_pre(file!(),line!(),column!());
            let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
            {LFORMAT_EXPAND}
            logwise::hidden::info_async_post(record).await;
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}

/// Generates synchronous warning-level logging code (all builds).
///
/// This macro creates warning-level log entries for suspicious conditions that don't
/// necessarily indicate errors but warrant attention. Unlike debug-only macros,
/// warnings are active in both debug and release builds.
///
/// # Syntax
/// ```
/// logwise::warn_sync!("format string");
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Always active (runtime cost incurred)
///
/// # Generated Code Pattern
/// ```
/// {
///     let mut record = logwise::hidden::warn_sync_pre(file!(), line!(), column!());
///     let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///     // ... formatter calls ...
///     logwise::hidden::warn_sync_post(record);
/// }
/// ```
///
/// # Use Cases
/// - Deprecated API usage
/// - Suboptimal configuration
/// - Recoverable error conditions
/// - Security-relevant events
///
/// # Examples
/// ```
/// // Configuration warnings
/// # let old_key = "old_setting";
/// # let new_key = "new_setting";
/// logwise::warn_sync!("Using deprecated config {key}, consider {alternative}",
///                     key=old_key, alternative=new_key);
///
/// // Performance warnings
/// # let size = 1024;
/// logwise::warn_sync!("Large payload detected: {size} bytes", size=size);
///
/// // Security warnings with privacy
/// logwise::warn_sync!("Failed login attempt from {ip}",
///                     ip=logwise::privacy::IPromiseItsNotPrivate("127.0.0.1"));
/// ```
#[proc_macro]
pub fn warn_sync(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        {{
            let mut record = logwise::hidden::warn_sync_pre(file!(),line!(),column!());

            let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);

            {LFORMAT_EXPAND}
            logwise::hidden::warn_sync_post(record);
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}

/// Generates code to begin a performance warning interval (all builds).
///
/// This macro starts a performance monitoring interval that will warn if an operation
/// takes longer than expected. It returns an interval guard that should be dropped
/// when the operation completes. Use `perfwarn!` for block-scoped intervals or
/// this macro for manual interval management.
///
/// # Syntax
/// ```
/// let value = 3;
/// let interval = logwise::perfwarn_begin!("operation description {param}", param=value);
/// // ... long-running operation ...
/// drop(interval); // Or let it drop automatically
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Always active (runtime cost incurred)
///
/// # Generated Code Pattern
/// ```ignore
/// {
///     let mut record = logwise::hidden::perfwarn_begin_pre(file!(), line!(), column!());
///     let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///     // ... formatter calls ...
///     logwise::hidden::perfwarn_begin_post(record, "operation description")
/// }
/// ```
///
/// # Examples
/// ```
/// // Manual interval management
/// let interval = logwise::perfwarn_begin!("Database query");
/// // let results = db.query(&sql).await;
/// drop(interval);
///
/// // Automatic cleanup with scope
/// {
///     let _interval = logwise::perfwarn_begin!("File processing {path}", path="/example.txt");
///     // process_large_file(&path);
/// } // interval automatically dropped here
/// ```
///
/// # See Also
/// - `perfwarn!` - For block-scoped performance intervals
#[proc_macro]
pub fn perfwarn_begin(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        {{
            let mut record = logwise::hidden::perfwarn_begin_pre(file!(),line!(),column!());
            let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
            {LFORMAT_EXPAND}
            logwise::hidden::perfwarn_begin_post(record,"{NAME}")
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output,
        NAME = lformat_result.name
    );
    src.parse().unwrap()
}

/// Generates code for a block-scoped performance warning interval (all builds).
///
/// This macro wraps a code block with performance monitoring, automatically managing
/// the interval lifecycle. It will warn if the block execution takes longer than expected.
/// The block's return value is preserved.
///
/// # Syntax
/// ```
/// # fn expensive_operation() -> i32 { 42 }
/// let value = 123;
/// let result = logwise::perfwarn!("operation description {param}", param=value, {
///     // code block to monitor
///     expensive_operation()
/// });
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Always active (runtime cost incurred)
///
/// # Generated Code Pattern
/// ```
/// {
///     let mut record = logwise::hidden::perfwarn_begin_pre(file!(), line!(), column!());
///     let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///     // ... formatter calls ...
///     let interval = logwise::hidden::perfwarn_begin_post(record, "operation description");
///     let result = { /* user block */ };
///     drop(interval);
///     result
/// }
/// ```
///
/// # Examples
/// ```
/// # fn process<T>(_t: T) {}
/// # struct Database; impl Database { fn query(&self, _: &str) -> Vec<String> { vec![] } }
/// # let database = Database;
/// // Monitor a database operation
/// let users = logwise::perfwarn!("Loading users from {table}", table="users", {
///     database.query("SELECT * FROM users")
/// });
///
/// // Monitor file I/O
/// let path = "large_file.txt";
/// let file_size_mb = 100;
/// let content = logwise::perfwarn!("Reading large file {size} MB", size=file_size_mb, {
///     "fake file content".to_string()  // Avoiding actual file I/O in doctests
/// });
///
/// // With complex parameters  
/// let items = vec![1, 2, 3];
/// let result: Vec<()> = logwise::perfwarn!("Processing {count} items", count=items.len(), {
///     items.into_iter().map(|item| process(item)).collect()
/// });
/// ```
///
/// # See Also
/// - `perfwarn_begin!` - For manual interval management
#[proc_macro]
pub fn perfwarn(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let last_token = input.pop_back().expect("Expected block");
    let lformat_expand = lformat_impl(&mut input, "formatter".to_string());

    let group = match last_token {
        TokenTree::Group(g) => g,
        _ => return r#"compile_error!("Expected block")"#.parse().unwrap(),
    };
    if group.delimiter() != proc_macro::Delimiter::Brace {
        return r#"compile_error!("Expected block")"#.parse().unwrap();
    }

    let src = format!(
        r#"
        {{
            let mut record = logwise::hidden::perfwarn_begin_pre(file!(),line!(),column!());
            let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
            {LFORMAT_EXPAND}
            let interval = logwise::hidden::perfwarn_begin_post(record,"{NAME}");
            let result = {BLOCK};
            drop(interval);
            result
        }}
    "#,
        LFORMAT_EXPAND = lformat_expand.output,
        BLOCK = group.to_string(),
        NAME = lformat_expand.name
    );
    src.parse().unwrap()
}

/// Generates synchronous error-level logging code (all builds).
///
/// This macro creates error-level log entries for actual error conditions that should
/// be logged in Result error paths. Error logging is active in both debug and release
/// builds since errors are always relevant.
///
/// # Syntax
/// ```
/// let key = "example_value";
/// logwise::error_sync!("format string with {placeholders}", placeholders=key);
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Always active (runtime cost incurred)
///
/// # Generated Code Pattern
/// ```
/// {
///     let mut record = logwise::hidden::error_sync_pre(file!(), line!(), column!());
///     let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
///     // ... formatter calls ...
///     logwise::hidden::error_sync_post(record);
/// }
/// ```
///
/// # Use Cases
/// - I/O failures
/// - Network errors
/// - Validation failures
/// - Resource exhaustion
/// - External service errors
///
/// # Examples
/// ```
/// # fn example() -> Result<Vec<u8>, std::io::Error> {
/// # let path = std::path::Path::new("/example.txt");
/// // I/O error logging
/// let content = match std::fs::read(&path) {
///     Ok(content) => content,
///     Err(e) => {
///         logwise::error_sync!("Failed to read file {path}: {error}",
///                              path=path.to_string_lossy().to_string(), error=e.to_string());
///         return Err(e);
///     }
/// };
/// # Ok(content)
/// # }
///
/// // Database error with privacy
/// # let user_id = "user123";
/// # let db_error = "Connection timeout";
/// logwise::error_sync!("Database query failed for user {id}: {error}",
///                      id=logwise::privacy::LogIt(user_id), error=db_error);
///
/// // Network error
/// # let response_status = 500;
/// # let request_url = "https://api.example.com";
/// logwise::error_sync!("HTTP request failed: {status} {url}",
///                      status=response_status, url=request_url);
/// ```
#[proc_macro]
pub fn error_sync(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        {{
            let mut record = logwise::hidden::error_sync_pre(file!(),line!(),column!());

            let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);

            {LFORMAT_EXPAND}
            logwise::hidden::error_sync_post(record);
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}

/// Generates asynchronous error-level logging code (all builds).
///
/// This macro creates async error-level log entries for actual error conditions in
/// async contexts. Like the sync variant, it's designed for Result error paths and
/// is active in both debug and release builds.
///
/// # Syntax
/// ```
/// async fn example() {
///     let key = "example_value";
///     logwise::error_async!("format string with {placeholders}", placeholders=key);
/// }
/// ```
///
/// # Build Configuration
/// - **Debug builds**: Always active
/// - **Release builds**: Always active (runtime cost incurred)
///
/// # Examples
/// ```
/// async fn read_config() -> Result<String, std::io::Error> {
///     let config_path = "/config.toml";
///     
///     // Simulate config reading with error handling
///     match std::fs::read_to_string(config_path) {
///         Ok(content) => Ok(content),
///         Err(e) => {
///             logwise::error_async!("Config read failed {path}: {error}",
///                                   path=config_path, error=logwise::privacy::LogIt(&e));
///             Err(e)
///         }
///     }
/// }
/// ```
///
/// ```
/// async fn example() {
///     let retry_count = 3;
///     let last_error = "Timeout";
///     logwise::error_async!("API request failed after {attempts} attempts: {error}",
///                           attempts=retry_count, error=last_error);
/// }
/// ```
#[proc_macro]
pub fn error_async(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        {{
            let mut record = logwise::hidden::error_sync_pre(file!(),line!(),column!());

            let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);

            {LFORMAT_EXPAND}
            logwise::hidden::error_async_post(record).await;
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}
