// SPDX-License-Identifier: MIT OR Apache-2.0

use logwise_runtime::Level;
use logwise_runtime::context::Context;

#[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn test_lazily_created_root_and_first_child_have_distinct_context_ids() {
    let lazy_root = Context::current();
    let first_child = Context::new_task(
        Some(lazy_root.clone()),
        "first_child".to_string(),
        Level::Info,
        false,
    );

    assert_ne!(
        lazy_root.context_id(),
        first_child.context_id(),
        "ContextID is documented as unique, including for the lazy root context"
    );
}
