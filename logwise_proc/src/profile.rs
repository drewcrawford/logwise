// SPDX-License-Identifier: MIT OR Apache-2.0

//! Entry points for the `Profile` level: `profile_begin!("fmt {k}", k = v)` opens a timed
//! interval and returns its guard, while `profile_sync!` / `profile_async!` log a point
//! event in the usual `pre` / formatter / `post` shape.
//!
//! Unlike `perfwarn`, `profile_begin_pre` also hands back an id that must be threaded
//! through to `profile_begin_post` so the two halves of an interval can be correlated;
//! both arms of the `log_enabled!(Level::Profile)` branch call the pair and return an
//! interval, so timing survives even when the message is not formatted. Profile output is
//! never compiled out, and like `mandatory` it is meant to be temporary.

use crate::parser::lformat_impl;
use proc_macro::TokenStream;
use std::collections::VecDeque;

pub fn profile_begin_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        {{
            if logwise::log_enabled!(logwise::Level::Profile) {{
                let (__logwise_id, mut __logwise_record) = logwise::hidden::profile_begin_pre(file!(),line!(),column!());
                let mut __logwise_formatter = logwise::hidden::PrivateFormatter::new(&mut __logwise_record);
                {LFORMAT_EXPAND}
                logwise::hidden::profile_begin_post(__logwise_id, __logwise_record, "{NAME}")
            }} else {{
                let (__logwise_id, __logwise_record) = logwise::hidden::profile_begin_pre(file!(),line!(),column!());
                logwise::hidden::profile_begin_post(__logwise_id, __logwise_record, "{NAME}")
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output,
        NAME = lformat_result.name
    );
    src.parse().unwrap()
}

pub fn profile_sync_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        {{
            if logwise::log_enabled!(logwise::Level::Profile) {{
                let mut __logwise_record = logwise::hidden::profile_sync_pre(file!(),line!(),column!());

                let mut __logwise_formatter = logwise::hidden::PrivateFormatter::new(&mut __logwise_record);

                {LFORMAT_EXPAND}
                logwise::hidden::profile_sync_post(__logwise_record);
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}

pub fn profile_async_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        {{
            if logwise::log_enabled!(logwise::Level::Profile) {{
                let mut __logwise_record = logwise::hidden::profile_sync_pre(file!(),line!(),column!());

                let mut __logwise_formatter = logwise::hidden::PrivateFormatter::new(&mut __logwise_record);

                {LFORMAT_EXPAND}
                logwise::hidden::profile_async_post(__logwise_record).await;
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}
