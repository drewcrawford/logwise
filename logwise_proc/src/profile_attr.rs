// SPDX-License-Identifier: MIT OR Apache-2.0
use proc_macro::{Delimiter, TokenStream, TokenTree};

/// Implementation of the `#[profile]` attribute macro.
///
/// Transforms a function to automatically profile its execution time by wrapping
/// the body with a `ProfileInterval` guard.
pub fn profile_attr_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut tokens: Vec<TokenTree> = item.into_iter().collect();

    // Find the function name and body
    let mut fn_name: Option<String> = None;
    let mut body_idx: Option<usize> = None;

    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Ident(ident) if ident.to_string() == "fn" => {
                // Next non-punct token should be the function name
                if i + 1 < tokens.len() {
                    if let TokenTree::Ident(name) = &tokens[i + 1] {
                        fn_name = Some(name.to_string());
                    }
                }
            }
            TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
                // This is the function body
                body_idx = Some(i);
                break;
            }
            _ => {}
        }
        i += 1;
    }

    let fn_name = match fn_name {
        Some(name) => name,
        None => {
            return "compile_error!(\"#[profile] can only be applied to functions\")"
                .parse()
                .unwrap()
        }
    };

    let body_idx = match body_idx {
        Some(idx) => idx,
        None => {
            return "compile_error!(\"#[profile] requires a function with a body\")"
                .parse()
                .unwrap()
        }
    };

    // Extract the original body
    let original_body = if let TokenTree::Group(g) = &tokens[body_idx] {
        g.stream()
    } else {
        return "compile_error!(\"Expected function body\")".parse().unwrap();
    };

    // Build the new body with profiling
    // We use profile_begin_pre + profile_begin_post directly, adding the name to the record
    let new_body_src = format!(
        r#"{{
            let _logwise_profile_guard = {{
                let (id, mut record) = logwise::hidden::profile_begin_pre(file!(), line!(), column!());
                let name = concat!(module_path!(), "::", "{fn_name}");
                record.log(name);
                logwise::hidden::profile_begin_post(id, record, name)
            }};
            {{ {original_body} }}
        }}"#,
        fn_name = fn_name,
        original_body = original_body
    );

    let new_body: TokenStream = new_body_src.parse().unwrap();
    let new_body_group = new_body.into_iter().next().unwrap();

    // Replace the old body with the new one
    tokens[body_idx] = new_body_group;

    tokens.into_iter().collect()
}
