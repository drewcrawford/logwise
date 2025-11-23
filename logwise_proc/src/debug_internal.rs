use crate::parser::lformat_impl;
use proc_macro::TokenStream;
use std::collections::VecDeque;

pub fn debuginternal_sync_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)] {{
            if logwise::log_enabled!(logwise::Level::DebugInternal, || crate::__LOGWISE_DOMAIN.is_internal()) {{
                    let mut record = logwise::hidden::debuginternal_pre(file!(),line!(),column!());
                    let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);
                    {LFORMAT_EXPAND}
                    logwise::hidden::debuginternal_sync_post(record);
           }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );
    src.parse().unwrap()
}

pub fn debuginternal_async_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        #[cfg(debug_assertions)] {{
            if logwise::log_enabled!(logwise::Level::DebugInternal, || crate::__LOGWISE_DOMAIN.is_internal()) {{
               let mut record = logwise::hidden::debuginternal_pre(file!(),line!(),column!());
                let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);

                {LFORMAT_EXPAND}
                logwise::hidden::debuginternal_async_post(record).await;
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}
