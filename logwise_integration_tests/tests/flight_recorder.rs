// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use logwise::{
    Class, ContextToken, Detail, FieldMetadata, Kind, Metadata, Privacy, Severity, ValueRef,
};
use logwise_runtime::{
    DetailLevel, EventSink, Filter, FlightCursor, FlightRecorder, ProjectedEvent, ProjectedField,
    RecorderView,
};

#[cfg(not(target_arch = "wasm32"))]
fn spawn<F, T>(work: F) -> std::thread::JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    std::thread::spawn(work)
}

#[cfg(target_arch = "wasm32")]
fn spawn<F, T>(work: F) -> wasm_lite_std::JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    wasm_lite_std::spawn(work)
}

fn yield_now() {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::yield_now();
    #[cfg(target_arch = "wasm32")]
    wasm_lite_std::yield_now();
}

static VALUE_FIELD: FieldMetadata = FieldMetadata::new("value", Privacy::SupportSafe, Detail::Core);
static DIRECT_FIELDS: &[FieldMetadata] = &[VALUE_FIELD];
static DIRECT_METADATA: Metadata = Metadata {
    event_name: "integration.flight.direct",
    package: "logwise_integration_tests",
    target: "flight_recorder",
    module: "flight_recorder",
    domain: None,
    severity: Severity::Debug,
    class: Class::Forensic,
    kind: Kind::Event,
    location: None,
    fields: DIRECT_FIELDS,
};

fn record_value(recorder: &FlightRecorder, value: u64) {
    recorder.emit(ProjectedEvent {
        metadata: &DIRECT_METADATA,
        context: ContextToken::NONE,
        fields: vec![ProjectedField {
            name: "value",
            privacy: Privacy::SupportSafe,
            detail: Detail::Core,
            value: ValueRef::U64(value),
        }],
        message: None,
        omitted_fields: 0,
    });
}

struct ReentrantDebug;

impl fmt::Debug for ReentrantDebug {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        logwise::event!("integration.flight.resilient.inner", value = local(1_u8));
        formatter.write_str("reentrant")
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct PanicDebug;

#[cfg(not(target_arch = "wasm32"))]
impl fmt::Debug for PanicDebug {
    fn fmt(&self, _formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("intentional recorder formatter panic")
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test(worker))]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn bounded_structured_history_is_queryable_and_privacy_projected() {
    let recorder = FlightRecorder::with_shards(3, 4, 1);
    for value in 0..5 {
        record_value(&recorder, value);
    }
    let read = recorder.read_since(FlightCursor(0), RecorderView::Local);
    assert!(read.is_complete());
    assert_eq!(read.records.len(), 3);
    assert_eq!(read.records[0].sequence, 3);
    assert_eq!(read.records[2].sequence, 5);
    assert_eq!(read.next_cursor, FlightCursor(5));
    assert_eq!(read.overwritten_total, 2);
    assert_eq!(read.dropped_total, 0);
    assert!(
        read.records[0]
            .to_string()
            .contains("integration.flight.direct")
    );

    let no_new_records = recorder.read_since(read.next_cursor, RecorderView::Local);
    assert!(no_new_records.records.is_empty());
    assert!(no_new_records.next_cursor >= read.next_cursor);

    let runtime = logwise_runtime::init().expect("install runtime");
    let projected = Arc::new(FlightRecorder::with_shards(8, 4, 1));
    let projected_id = runtime.add_local_sink(
        projected.clone(),
        Filter::new().event("integration.flight.projected"),
        DetailLevel::Core,
    );
    let detail_evaluations = AtomicUsize::new(0);
    logwise::event!(
        "integration.flight.projected",
        public = support("public-value"),
        private = local("private-value"),
        secret = secret("secret-value"),
        detail expensive = local({
            detail_evaluations.fetch_add(1, Ordering::Relaxed);
            "expensive-value"
        }),
    );
    assert_eq!(detail_evaluations.load(Ordering::Relaxed), 0);

    let local = projected.tail(1, RecorderView::Local);
    assert_eq!(local.records[0].event.fields.len(), 2);
    assert_eq!(local.records[0].event.truncated_fields, 2);
    assert_eq!(local.records[0].event.omitted_fields, 2);
    let remote = projected.tail(1, RecorderView::Remote);
    assert_eq!(remote.records[0].event.fields.len(), 1);
    assert_eq!(remote.records[0].event.fields[0].name, "public");
    assert_eq!(remote.records[0].event.omitted_fields, 3);
    assert_eq!(remote.truncated_fields_total, 2);
    assert!(runtime.remove_sink(projected_id));

    let text = Arc::new(FlightRecorder::with_shards(2, 128, 1));
    let text_id = runtime.add_local_sink(
        text.clone(),
        Filter::new().event("logwise.adhoc"),
        DetailLevel::Core,
    );
    logwise::log!("local text {}", 42);
    assert!(
        text.tail(1, RecorderView::Local).records[0]
            .event
            .message
            .is_some()
    );
    assert!(
        text.tail(1, RecorderView::Remote).records[0]
            .event
            .message
            .is_none()
    );
    assert!(runtime.remove_sink(text_id));

    let resilient = Arc::new(FlightRecorder::with_shards(4, 64, 1));
    let resilient_id = runtime.add_local_sink(
        resilient.clone(),
        Filter::new().event("integration.flight.resilient"),
        DetailLevel::Core,
    );
    let reentrant_before = runtime.delivery_stats().reentrant_events_dropped;
    let value = ReentrantDebug;
    logwise::event!(
        "integration.flight.resilient.reentrant",
        value = local(ValueRef::debug(&value)),
    );
    assert_eq!(
        runtime.delivery_stats().reentrant_events_dropped,
        reentrant_before + 1
    );
    assert_eq!(resilient.stats().accepted, 1);

    #[cfg(not(target_arch = "wasm32"))]
    {
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let panics_before = runtime.delivery_stats().sink_panics;
        let value = PanicDebug;
        logwise::event!(
            "integration.flight.resilient.panic",
            value = local(ValueRef::debug(&value)),
        );
        assert_eq!(runtime.delivery_stats().sink_panics, panics_before + 1);
        logwise::event!(
            "integration.flight.resilient.after_panic",
            value = local(7_u8),
        );
        assert_eq!(resilient.stats().accepted, 2);
        std::panic::set_hook(previous_hook);
    }
    assert!(runtime.remove_sink(resilient_id));
}

#[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test(worker))]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn concurrent_writers_and_readers_preserve_order_and_accounting() {
    let recorder = Arc::new(FlightRecorder::with_shards(32, 64, 4));
    let running = Arc::new(AtomicBool::new(true));
    let reader_recorder = recorder.clone();
    let reader_running = running.clone();
    let reader = spawn(move || {
        while reader_running.load(Ordering::Acquire) {
            let read = reader_recorder.read_since(FlightCursor(0), RecorderView::Local);
            assert!(
                read.records
                    .windows(2)
                    .all(|pair| pair[0].sequence < pair[1].sequence)
            );
            yield_now();
        }
    });

    let writers: Vec<_> = (0..4)
        .map(|thread| {
            let recorder = recorder.clone();
            spawn(move || {
                for value in 0..64 {
                    record_value(&recorder, thread * 64 + value);
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().expect("writer thread");
    }
    running.store(false, Ordering::Release);
    reader.join().expect("reader thread");

    let final_read = recorder.read_since(FlightCursor(0), RecorderView::Local);
    assert!(final_read.is_complete());
    assert!(final_read.records.len() <= 32);
    assert!(
        final_read
            .records
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );
    let stats = recorder.stats();
    assert_eq!(stats.accepted + stats.dropped, 256);
    assert_eq!(final_read.next_cursor, FlightCursor(stats.accepted));
    assert_eq!(final_read.dropped_total, stats.dropped);
    assert_eq!(final_read.overwritten_total, stats.overwritten);
}
