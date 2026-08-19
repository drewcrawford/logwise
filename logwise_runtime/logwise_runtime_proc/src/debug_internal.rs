// SPDX-License-Identifier: MIT OR Apache-2.0

//! Entry points for `debuginternal_sync!` / `debuginternal_async!`, the level a library
//! author uses to debug their own crate.
//!
//! Like `trace`, the expansion is wrapped in `#[cfg(debug_assertions)]` and so is absent
//! from release builds, but the runtime `log_enabled!(Level::DebugInternal)` check passes
//! by default in any crate that invoked `declare_logging_domain!()` at its root, rather
//! than requiring per-thread activation.

use crate::parser::lformat_impl;
use proc_macro::TokenStream;
use std::collections::VecDeque;

pub fn debuginternal_sync_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)] {{
            if logwise_runtime::log_enabled!(logwise_runtime::Level::DebugInternal) {{
                    let mut __logwise_record = logwise_runtime::hidden::debuginternal_pre(file!(),line!(),column!());
                    let mut __logwise_formatter = logwise_runtime::hidden::PrivateFormatter::new(&mut __logwise_record);
                    {LFORMAT_EXPAND}
                    logwise_runtime::hidden::debuginternal_sync_post(__logwise_record);
           }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );
    src.parse().unwrap()
}

pub fn debuginternal_async_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "__logwise_formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)] {{
            if logwise_runtime::log_enabled!(logwise_runtime::Level::DebugInternal) {{
               let mut __logwise_record = logwise_runtime::hidden::debuginternal_pre(file!(),line!(),column!());
                let mut __logwise_formatter = logwise_runtime::hidden::PrivateFormatter::new(&mut __logwise_record);

                {LFORMAT_EXPAND}
                logwise_runtime::hidden::debuginternal_async_post(__logwise_record).await;
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}
