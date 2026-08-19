// SPDX-License-Identifier: MIT OR Apache-2.0

//! Entry points for `trace_sync!` / `trace_async!`, the noisiest level.
//!
//! Both take `"literal {key}", key = expr` and expand to a `#[cfg(debug_assertions)]`
//! block guarded by `log_enabled!(Level::Trace)`, so the call disappears entirely from
//! release builds and, in debug builds, still costs nothing unless the thread opted in
//! via `Context::begin_trace()`. The async variant shares `trace_sync_pre` for the
//! prelude and only differs in awaiting `trace_async_post`.

use crate::parser::lformat_impl;
use proc_macro::TokenStream;
use std::collections::VecDeque;

pub fn trace_sync_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)]
        {{
            if logwise_runtime::log_enabled!(logwise_runtime::Level::Trace) {{
                let mut __logwise_record = logwise_runtime::hidden::trace_sync_pre(file!(),line!(),column!());

                let mut __logwise_formatter = logwise_runtime::hidden::PrivateFormatter::new(&mut __logwise_record);

                {LFORMAT_EXPAND}
                logwise_runtime::hidden::trace_sync_post(__logwise_record);
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}

pub fn trace_async_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)]
        {{
            if logwise_runtime::log_enabled!(logwise_runtime::Level::Trace) {{
                let mut __logwise_record = logwise_runtime::hidden::trace_sync_pre(file!(),line!(),column!());

                let mut __logwise_formatter = logwise_runtime::hidden::PrivateFormatter::new(&mut __logwise_record);

                {LFORMAT_EXPAND}
                logwise_runtime::hidden::trace_async_post(__logwise_record).await;
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}
