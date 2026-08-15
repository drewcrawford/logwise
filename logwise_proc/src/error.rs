// SPDX-License-Identifier: MIT OR Apache-2.0

//! Entry points for `error_sync!` / `error_async!`, used on `Result` error paths where
//! an operation actually failed.
//!
//! Compiled into every profile: there is no `#[cfg(debug_assertions)]` gate and
//! `log_enabled!(Level::Error)` is constant-`true`, so the message text and its value
//! expressions must be release-safe. The async variant reuses `error_sync_pre` to build
//! the record and differs only by awaiting `error_async_post`.

use crate::parser::lformat_impl;
use proc_macro::TokenStream;
use std::collections::VecDeque;

pub fn error_sync_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        {{
            if logwise::log_enabled!(logwise::Level::Error) {{
                let mut __logwise_record = logwise::hidden::error_sync_pre(file!(),line!(),column!());

                let mut __logwise_formatter = logwise::hidden::PrivateFormatter::new(&mut __logwise_record);

                {LFORMAT_EXPAND}
                logwise::hidden::error_sync_post(__logwise_record);
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}

pub fn error_async_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        {{
            if logwise::log_enabled!(logwise::Level::Error) {{
                let mut __logwise_record = logwise::hidden::error_sync_pre(file!(),line!(),column!());

                let mut __logwise_formatter = logwise::hidden::PrivateFormatter::new(&mut __logwise_record);

                {LFORMAT_EXPAND}
                logwise::hidden::error_async_post(__logwise_record).await;
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}
