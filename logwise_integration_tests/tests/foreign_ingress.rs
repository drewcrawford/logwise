// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;

use logwise::Privacy;
use logwise_runtime::{
    DetailLevel, Filter, FlightCursor, FlightRecorder, ForeignOrigin, InMemorySink, OverflowPolicy,
    RecorderView, foreign_text,
};

#[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test(worker))]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn foreign_text_is_local_only_and_origin_marked() {
    let runtime = logwise_runtime::init().expect("install runtime");
    let local = Arc::new(FlightRecorder::with_shards(16, 512, 1));
    let local_id = runtime.add_local_sink(
        local.clone(),
        Filter::new().event("foreign.text"),
        DetailLevel::Core,
    );
    let remote = Arc::new(InMemorySink::new(16, 512, OverflowPolicy::DropNewest));
    let remote_id = runtime.add_remote_sink(
        remote.clone(),
        Filter::new().event("foreign.text"),
        DetailLevel::Core,
    );

    foreign_text(ForeignOrigin::RustStdout, "foreign stdout marker");
    logwise_runtime_wasm::ingest_console(
        logwise_runtime_wasm::ConsoleOrigin::Warn,
        "foreign console marker",
    );

    #[cfg(not(target_arch = "wasm32"))]
    {
        use logwise_runtime::install_panic_hook;

        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let registration = install_panic_hook();
        let _ = std::panic::catch_unwind(|| panic!("foreign panic marker"));
        registration.restore();
        std::panic::set_hook(original_hook);
    }

    #[cfg(unix)]
    {
        use std::io::Write;

        use logwise_runtime::{NativeFd, NativeFdCapture};

        let capture = NativeFdCapture::start(NativeFd::Stderr).expect("start FD capture");
        std::io::stderr()
            .write_all(b"foreign fd marker")
            .expect("write redirected stderr");
        let captured = capture.finish().expect("finish FD capture");
        assert_eq!(captured, "foreign fd marker");
    }

    let read = local.read_since(FlightCursor(0), RecorderView::Local);
    #[cfg(all(not(target_arch = "wasm32"), unix))]
    assert_eq!(read.records.len(), 4);
    #[cfg(all(not(target_arch = "wasm32"), not(unix)))]
    assert_eq!(read.records.len(), 3);
    #[cfg(target_arch = "wasm32")]
    assert_eq!(read.records.len(), 2);
    for record in &read.records {
        assert_eq!(record.event.metadata.event_name, "foreign.text");
        assert_eq!(record.event.fields.len(), 2);
        assert!(
            record
                .event
                .fields
                .iter()
                .all(|field| field.privacy == Privacy::LocalOnly)
        );
    }
    assert!(read.records.iter().any(|record| {
        record.event.fields.iter().any(|field| {
            field.name == "origin"
                && field.value == logwise_runtime::OwnedValue::String("js.console.warn".into())
        })
    }));

    let projected = local.read_since(FlightCursor(0), RecorderView::Remote);
    assert!(
        projected
            .records
            .iter()
            .all(|record| record.event.fields.is_empty())
    );
    assert_eq!(remote.stats().accepted, 0);
    assert!(runtime.remove_sink(remote_id));
    assert!(runtime.remove_sink(local_id));
}
