// SPDX-License-Identifier: MIT OR Apache-2.0

//! The three `PerfWarn` interval entry points, which time a region rather than logging a
//! point event: `perfwarn_begin!("fmt {k}", k = v)` returns a guard that reports on drop,
//! `perfwarn!("fmt", { .. })` takes a trailing brace group and scopes that guard to the
//! block, and `perfwarn_begin_if!(threshold, "fmt", ..)` parses a leading expression up
//! to the first comma and reports only when the interval exceeds it.
//!
//! Every branch of the expansion yields an interval, including the one taken when
//! `log_enabled!(Level::PerfWarn)` is false, so the macro's type does not depend on
//! whether logging is on. The format string doubles as the interval's name, taken from
//! `LFormatResult::name` and spliced into the expansion as a string literal.

use crate::parser::lformat_impl;
use proc_macro::{TokenStream, TokenTree};
use std::collections::VecDeque;

pub fn perfwarn_begin_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        {{
            if logwise::log_enabled!(logwise::Level::PerfWarn) {{
                let mut __logwise_record = logwise::hidden::perfwarn_begin_pre(file!(),line!(),column!());
                let mut __logwise_formatter = logwise::hidden::PrivateFormatter::new(&mut __logwise_record);
                {LFORMAT_EXPAND}
                logwise::hidden::perfwarn_begin_post(__logwise_record,"{NAME}")
            }} else {{
                logwise::hidden::perfwarn_begin_post(
                    logwise::hidden::perfwarn_begin_pre(file!(),line!(),column!()),
                    "{NAME}",
                )
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output,
        NAME = lformat_result.name
    );
    src.parse().unwrap()
}

pub fn perfwarn_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let last_token = match input.pop_back() {
        Some(t) => t,
        //an empty invocation is a user error, not a reason to panic the compiler
        None => return r#"compile_error!("Expected block")"#.parse().unwrap(),
    };
    let lformat_expand = lformat_impl(&mut input, "__logwise_formatter".to_string());

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
            let __logwise_interval = if logwise::log_enabled!(logwise::Level::PerfWarn) {{
                let mut __logwise_record = logwise::hidden::perfwarn_begin_pre(file!(),line!(),column!());
                let mut __logwise_formatter = logwise::hidden::PrivateFormatter::new(&mut __logwise_record);
                {LFORMAT_EXPAND}
                Some(logwise::hidden::perfwarn_begin_post(__logwise_record,"{NAME}"))
            }} else {{
                None
            }};
            let __logwise_result = {BLOCK};
            drop(__logwise_interval);
            __logwise_result
        }}
    "#,
        LFORMAT_EXPAND = lformat_expand.output,
        BLOCK = group,
        NAME = lformat_expand.name
    );
    src.parse().unwrap()
}

pub fn perfwarn_begin_if_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();

    // Parse threshold expression (first argument)
    // We need to consume tokens until the first comma
    let mut threshold_tokens = TokenStream::new();
    loop {
        match input.pop_front() {
            Some(TokenTree::Punct(p)) if p.as_char() == ',' => break,
            Some(t) => threshold_tokens.extend(std::iter::once(t)),
            None => return r#"compile_error!("Expected threshold argument")"#.parse().unwrap(),
        }
    }

    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        {{
            if logwise::log_enabled!(logwise::Level::PerfWarn) {{
                let mut __logwise_record = logwise::hidden::perfwarn_begin_if_pre(file!(),line!(),column!());
                let mut __logwise_formatter = logwise::hidden::PrivateFormatter::new(&mut __logwise_record);
                {LFORMAT_EXPAND}
                logwise::hidden::perfwarn_begin_if_post(__logwise_record, "{NAME}", {THRESHOLD})
            }} else {{
                logwise::hidden::perfwarn_begin_if_post(
                    logwise::hidden::perfwarn_begin_if_pre(file!(),line!(),column!()),
                    "{NAME}",
                    {THRESHOLD}
                )
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output,
        NAME = lformat_result.name,
        THRESHOLD = threshold_tokens.to_string()
    );
    src.parse().unwrap()
}
