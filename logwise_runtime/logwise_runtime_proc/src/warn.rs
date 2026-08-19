// SPDX-License-Identifier: MIT OR Apache-2.0

//! Entry point for `warn_sync!`, for suspicious-but-recoverable conditions.
//!
//! Unlike the debug-only levels there is no `#[cfg(debug_assertions)]` gate, and
//! `log_enabled!(Level::Warning)` is constant-`true`, so the record is always built and
//! any filtering is left to the installed loggers. This module is sync-only -- there is
//! deliberately no `warn_async!` counterpart to the other levels' async variants.

use crate::parser::lformat_impl;
use proc_macro::TokenStream;
use std::collections::VecDeque;

pub fn warn_sync_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        {{
            if logwise_runtime::log_enabled!(logwise_runtime::Level::Warning) {{
                let mut __logwise_record = logwise_runtime::hidden::warn_sync_pre(file!(),line!(),column!());

                let mut __logwise_formatter = logwise_runtime::hidden::PrivateFormatter::new(&mut __logwise_record);

                {LFORMAT_EXPAND}
                logwise_runtime::hidden::warn_sync_post(__logwise_record);
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}
