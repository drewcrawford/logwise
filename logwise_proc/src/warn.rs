use crate::parser::lformat_impl;
use proc_macro::TokenStream;
use std::collections::VecDeque;

pub fn warn_sync_impl(input: TokenStream) -> TokenStream {
    let mut input: VecDeque<_> = input.into_iter().collect();
    let lformat_result = lformat_impl(&mut input, "formatter".to_string());
    let src = format!(
        r#"
        {{
            if logwise::log_enabled!(logwise::Level::Warning) {{
                let mut record = logwise::hidden::warn_sync_pre(file!(),line!(),column!());

                let mut formatter = logwise::hidden::PrivateFormatter::new(&mut record);

                {LFORMAT_EXPAND}
                logwise::hidden::warn_sync_post(record);
            }}
        }}
    "#,
        LFORMAT_EXPAND = lformat_result.output
    );

    src.parse().unwrap()
}
