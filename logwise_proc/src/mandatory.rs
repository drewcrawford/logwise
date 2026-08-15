// SPDX-License-Identifier: MIT OR Apache-2.0

//! Entry points for `mandatory_sync!` / `mandatory_async!`, printf-style debugging that
//! nothing is allowed to silence.
//!
//! The `log_enabled!(Level::Mandatory)` guard in the expansion is constant-`true` in
//! every build profile and cannot be turned off by a logging domain, which is the point:
//! it is for debugging hostile environments where the other levels are compiled out.
//! That is also why these calls are meant to be deleted before committing, rather than
//! left in place like `warn`/`error`.

use crate::parser::lformat_impl;
use proc_macro::TokenStream;
use std::collections::VecDeque;

pub fn mandatory_sync_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        {{
            if logwise::log_enabled!(logwise::Level::Mandatory) {{
                let mut __logwise_record = logwise::hidden::mandatory_sync_pre(file!(),line!(),column!());

                let mut __logwise_formatter = logwise::hidden::PrivateFormatter::new(&mut __logwise_record);

                {LFORMAT_EXPAND}
                logwise::hidden::mandatory_sync_post(__logwise_record);
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}

pub fn mandatory_async_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        {{
            if logwise::log_enabled!(logwise::Level::Mandatory) {{
                let mut __logwise_record = logwise::hidden::mandatory_sync_pre(file!(),line!(),column!());

                let mut __logwise_formatter = logwise::hidden::PrivateFormatter::new(&mut __logwise_record);

                {LFORMAT_EXPAND}
                logwise::hidden::mandatory_async_post(__logwise_record).await;
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}
