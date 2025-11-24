// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::parser::lformat_impl;
use proc_macro::TokenStream;
use std::collections::VecDeque;

pub fn trace_sync_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)]
        {{
            if logwise::log_enabled!(logwise::Level::Trace) {{
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

pub fn trace_async_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)]
        {{
            if logwise::log_enabled!(logwise::Level::Trace) {{
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
