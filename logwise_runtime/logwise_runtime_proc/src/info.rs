// SPDX-License-Identifier: MIT OR Apache-2.0

//! Entry points for `info_sync!` / `info_async!`, the level aimed at the *users* of a
//! crate rather than at its author.
//!
//! Still debug-only -- the emitted block carries `#[cfg(debug_assertions)]`, so release
//! builds pay nothing -- but unlike `trace` it needs no per-thread opt-in beyond the
//! `log_enabled!(Level::Info)` check. Both variants build the record through
//! `info_sync_pre` and differ only in which `_post` they call.

use crate::parser::lformat_impl;
use proc_macro::TokenStream;
use std::collections::VecDeque;

pub fn info_sync_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)]
        {{
            if logwise_runtime::log_enabled!(logwise_runtime::Level::Info) {{
                let mut __logwise_record = logwise_runtime::hidden::info_sync_pre(file!(),line!(),column!());

                let mut __logwise_formatter = logwise_runtime::hidden::PrivateFormatter::new(&mut __logwise_record);

                {LFORMAT_EXPAND}
                logwise_runtime::hidden::info_sync_post(__logwise_record);
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}

pub fn info_async_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)] {{
            if logwise_runtime::log_enabled!(logwise_runtime::Level::Info) {{
                let mut __logwise_record = logwise_runtime::hidden::info_sync_pre(file!(),line!(),column!());
                let mut __logwise_formatter = logwise_runtime::hidden::PrivateFormatter::new(&mut __logwise_record);
                {LFORMAT_EXPAND}
                logwise_runtime::hidden::info_async_post(__logwise_record).await;
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}
