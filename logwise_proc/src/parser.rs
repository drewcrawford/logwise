// SPDX-License-Identifier: MIT OR Apache-2.0
//SPDX-License-Identifier: MIT OR Apache-2.0

use proc_macro::{Spacing, TokenStream, TokenTree};
use std::collections::{HashSet, VecDeque};

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
/// It preserves the original Rust tokens, including the spacing required between
/// adjacent identifiers and keywords.
///
/// # Arguments
/// * `input` - Mutable reference to the token stream being parsed
///
/// # Returns
/// * `TokenStream` - The complete value expression
///
/// # Examples
/// ```ignore
/// # // ignore because: This shows pseudo-code for token stream parsing, not actual runnable code
/// // For input: `user.name.clone(), next_param = value`
/// // Returns: "user.name.clone()".to_string()
/// // Consumes everything until the comma
/// ```
fn parse_value(input: &mut VecDeque<TokenTree>) -> TokenStream {
    //basically we go until we get a , or end.
    let mut value = TokenStream::new();
    //Depth of unclosed generic argument lists, opened by a turbofish (`::<`) or by
    //the leading `<` of a qualified path (`<T as Trait<A, B>>::f()`). A comma in
    //there separates generic arguments, not macro arguments, so it must not end the
    //value. Commas nested in (), [] or {} arrive inside a Group token and never
    //reach this loop.
    let mut generic_depth = 0usize;
    //number of consecutive `:` puncts, used to spot the `::<` that opens a turbofish
    let mut colon_run = 0usize;
    //previous punct, used to tell the `>` of `->`/`=>` from a closing angle bracket
    let mut prev_punct: Option<(char, Spacing)> = None;
    loop {
        let token = match input.pop_front() {
            Some(token) => token,
            None => return value,
        };
        if let TokenTree::Punct(p) = &token {
            let c = p.as_char();
            match c {
                ',' if generic_depth == 0 => return value,
                '<' if generic_depth > 0 || colon_run >= 2 || value.is_empty() => {
                    generic_depth += 1;
                }
                '>' if generic_depth > 0
                    && !matches!(prev_punct, Some(('-' | '=', Spacing::Joint))) =>
                {
                    generic_depth -= 1;
                }
                _ => {}
            }
            colon_run = if c == ':' { colon_run + 1 } else { 0 };
            prev_punct = Some((c, p.spacing()));
        } else {
            colon_run = 0;
            prev_punct = None;
        }
        value.extend(std::iter::once(token));
    }
}

/// Builds an ordered list of key-value pairs from the remaining token stream.
///
/// This function processes the parameter list portion of logging macros, extracting
/// all key-value pairs that follow the format string. It expects the first token
/// to be a comma separator, then processes alternating key=value pairs.
///
/// # Arguments
/// * `input` - Mutable reference to the token stream containing key-value pairs
///
/// # Returns
/// * `Ok(Vec<(String, TokenStream)>)` - Successfully parsed key-value pairs
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
fn build_kvs(input: &mut VecDeque<TokenTree>) -> Result<Vec<(String, TokenStream)>, TokenStream> {
    let mut kvs = Vec::new();
    //first extract the comma.
    if input.is_empty() {
        return Ok(kvs);
    }
    match input.pop_front() {
        Some(TokenTree::Punct(p)) => {
            if p.as_char() != ',' {
                return Err(r#"compile_error!("Expected ','");"#.parse().unwrap());
            }
        }
        _ => {
            return Err(r#"compile_error!("Expected ','");"#.parse().unwrap());
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
        if kvs.iter().any(|(existing, _)| existing == &key) {
            //the key is stringified call-site tokens, so quote it rather than
            //splicing it into a literal raw
            return Err(
                format!(r#"compile_error!({:?});"#, format!("Duplicate key {key}"))
                    .parse()
                    .unwrap(),
            );
        }
        kvs.push((key, value));
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
                output: r#"compile_error!("lformat!() must be called with a string literal");"#
                    .parse()
                    .unwrap(),
                name: "".to_string(),
            };
        }
    };
    let format_string = match some_input {
        TokenTree::Literal(l) => {
            let out = l.to_string();
            if !out.starts_with('"') || !out.ends_with('"') {
                return LFormatResult {
                    output: r#"compile_error!("lformat!() must be called with a string literal");"#
                        .parse()
                        .unwrap(),
                    name: "".to_string(),
                };
            }
            out[1..out.len() - 1].to_string()
        }
        _ => {
            return LFormatResult {
                output: r#"compile_error!("lformat!() must be called with a string literal");"#
                    .parse()
                    .unwrap(),
                name: "".to_string(),
            };
        }
    };

    //parse kv section
    let key_values = match build_kvs(collect) {
        Ok(kvs) => kvs,
        Err(e) => {
            return LFormatResult {
                output: e,
                name: "".to_string(),
            };
        }
    };
    let mut used_keys = HashSet::new();
    //parse format string
    //holds the part of the string literal until the next {
    let mut source = String::new();
    //`format_string` is the *source* form of the literal, so escape sequences are
    //still spelled out (`\n`, `\u{1F600}`, ...) and literal text is re-emitted
    //verbatim into another string literal below. Escapes therefore round-trip, as
    //long as the scan never mistakes a brace belonging to `\u{...}` for a
    //placeholder delimiter.
    let chars: Vec<char> = format_string.chars().collect();
    let mut literal = String::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => {
                //copy the whole escape sequence verbatim, braces included
                literal.push('\\');
                i += 1;
                let Some(&escaped) = chars.get(i) else {
                    break;
                };
                literal.push(escaped);
                i += 1;
                if escaped == 'u' && chars.get(i) == Some(&'{') {
                    while let Some(&c) = chars.get(i) {
                        literal.push(c);
                        i += 1;
                        if c == '}' {
                            break;
                        }
                    }
                }
            }
            //escaped braces: emit one brace and consume both
            '{' if chars.get(i + 1) == Some(&'{') => {
                literal.push('{');
                i += 2;
            }
            '}' if chars.get(i + 1) == Some(&'}') => {
                literal.push('}');
                i += 2;
            }
            '{' => {
                if !literal.is_empty() {
                    //reference logger ident
                    source.push_str(&logger);
                    source.push_str(".write_literal(\"");
                    source.push_str(&literal);
                    source.push_str("\");\n");
                    literal.clear();
                }
                i += 1;
                let mut key = String::new();
                let mut closed = false;
                while let Some(&c) = chars.get(i) {
                    i += 1;
                    if c == '}' {
                        closed = true;
                        break;
                    }
                    key.push(c);
                }
                if !closed {
                    return LFormatResult {
                        output: r#"compile_error!("Expected '}'");"#.parse().unwrap(),
                        name: "".to_string(),
                    };
                }
                //write out the key
                source.push_str(&logger);
                source.push_str(".write_val(");
                let value = match key_values.iter().find(|(name, _)| name == &key) {
                    Some((_, value)) => value.to_string(),
                    None => {
                        return LFormatResult {
                            //the key is arbitrary text from the format string, so
                            //quote it rather than splicing it into a literal raw
                            output: format!(
                                r#"compile_error!({:?});"#,
                                format!("Key {key} not found")
                            )
                            .parse()
                            .unwrap(),
                            name: "".to_string(),
                        };
                    }
                };
                used_keys.insert(key);
                source.push_str(&value);
                source.push_str(");\n");
            }
            c => {
                literal.push(c);
                i += 1;
            }
        }
    }
    if !literal.is_empty() {
        source.push_str(&logger);
        source.push_str(".write_literal(\"");
        source.push_str(&literal);
        source.push_str("\");\n");
    }

    // Key/value arguments that are not interpolated are structured fields, not
    // dead syntax. Preserve them in call-site order instead of silently dropping
    // both the field and the value expression.
    for (key, value) in &key_values {
        if used_keys.contains(key) {
            continue;
        }
        source.push_str(&logger);
        source.push_str(".write_literal(");
        source.push_str(&format!("{:?}", format!(" {key}=")));
        source.push_str(");\n");
        source.push_str(&logger);
        source.push_str(".write_val(");
        source.push_str(&value.to_string());
        source.push_str(");\n");
    }

    return LFormatResult {
        output: source.parse().unwrap(),
        name: format_string,
    };
}
