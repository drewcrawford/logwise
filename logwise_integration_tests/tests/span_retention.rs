// SPDX-License-Identifier: MIT OR Apache-2.0

//! Completed spans are retained for a query that may never come, so the buffer
//! holding them has to be bounded like every other buffer in the runtime.

use logwise::Interest;

const SPANS: usize = 5_000;

#[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn completed_spans_do_not_accumulate_without_a_reader() {
    let runtime = logwise_runtime::init().expect("install runtime");
    runtime.set_interest(Interest::CORE_LOCAL);

    let before = runtime.delivery_stats().completed_spans_dropped;
    for _ in 0..SPANS {
        drop(logwise::span!("integration.spans.retention"));
    }

    let dropped = runtime.delivery_stats().completed_spans_dropped - before;
    let retained = runtime.take_completed_spans();
    assert!(
        retained.len() < SPANS,
        "nothing read these spans, so the runtime must not still be holding all {SPANS}"
    );
    assert_eq!(
        retained.len() + dropped as usize,
        SPANS,
        "every span is either retained or accounted as dropped"
    );
    assert!(
        retained
            .iter()
            .all(|span| span.event_name == "integration.spans.retention"),
        "the retained window is the most recent spans, not an arbitrary subset"
    );

    // Draining leaves room again.
    drop(logwise::span!("integration.spans.retention"));
    let after = runtime.take_completed_spans();
    assert_eq!(after.len(), 1);
    assert_eq!(
        runtime.delivery_stats().completed_spans_dropped - before,
        dropped
    );
}
