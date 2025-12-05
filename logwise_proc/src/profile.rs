// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::parser::lformat_impl;
use proc_macro::TokenStream;
use std::collections::VecDeque;

pub fn profile_begin_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        {{
            if logwise::log_enabled!(logwise::Level::Profile) {{
                let (id, mut record) = logwise::hidden::profile_begin_pre(file!(),line!(),column!());
                let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
                {LFORMAT_EXPAND}
                logwise::hidden::profile_begin_post(id, record, "{NAME}")
            }} else {{
                let (id, record) = logwise::hidden::profile_begin_pre(file!(),line!(),column!());
                logwise::hidden::profile_begin_post(id, record, "{NAME}")
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
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        {{
            if logwise::log_enabled!(logwise::Level::Profile) {{
                let mut record = logwise::hidden::profile_sync_pre(file!(),line!(),column!());

                let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);

                {LFORMAT_EXPAND}
                logwise::hidden::profile_sync_post(record);
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}

pub fn profile_async_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        {{
            if logwise::log_enabled!(logwise::Level::Profile) {{
                let mut record = logwise::hidden::profile_sync_pre(file!(),line!(),column!());

                let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);

                {LFORMAT_EXPAND}
                logwise::hidden::profile_async_post(record).await;
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}
