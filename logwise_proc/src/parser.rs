//SPDX-License-Identifier: MIT OR Apache-2.0

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
/// # // ignore because: This shows pseudo-code for token stream parsing, not actual runnable code
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
/// # // ignore because: This shows pseudo-code for token stream parsing, not actual runnable code
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
/// # // ignore because: This shows pseudo-code for token stream parsing, not actual runnable code
/// // After format string: , key1=value1, key2=value2, key3=complex_expr()
/// ```
///
/// # Examples
/// ```ignore
/// # // ignore because: This shows pseudo-code for token stream parsing, not actual runnable code
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
/// # // ignore because: This illustrates generated code output, not actual runnable code
/// formatter.write_literal("Hello, ");
/// formatter.write_val(username);
/// formatter.write_literal("!");
/// ```
pub struct LFormatResult {
    pub output: TokenStream,
    pub name: String,
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
/// # // ignore because: This shows pseudo-code for code generation, not actual runnable code
/// // Input: "Hello {name}!", name="world"
/// // Generates:
/// // formatter.write_literal("Hello ");
/// // formatter.write_val("world");
/// // formatter.write_literal("!");
/// ```
pub fn lformat_impl(collect: &mut VecDeque<TokenTree>, logger: String) -> LFormatResult {
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
