// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use logwise_runtime::{
    AsyncSink, DetailLevel, EventSink, Filter, InMemorySink, OverflowPolicy, OwnedEventWriter,
    OwnedProjectedEvent, ProjectedEvent, StructuredWriter,
};

struct SharedBytes(Arc<Mutex<Vec<u8>>>);

impl Write for SharedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct CountSink(AtomicUsize);

impl EventSink for CountSink {
    fn emit(&self, _event: ProjectedEvent<'_>) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct PanicSink;

#[cfg(not(target_arch = "wasm32"))]
impl EventSink for PanicSink {
    fn emit(&self, _event: ProjectedEvent<'_>) {
        panic!("intentional sink panic");
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct PanicWriter;

#[cfg(not(target_arch = "wasm32"))]
impl OwnedEventWriter for PanicWriter {
    fn write_event(&mut self, _event: &OwnedProjectedEvent) -> io::Result<()> {
        panic!("intentional writer panic");
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct ReentrantSink;

impl EventSink for ReentrantSink {
    fn emit(&self, _event: ProjectedEvent<'_>) {
        logwise::event!("integration.sinks.reentrant.inner", value = local(1_u8));
    }
}

struct LoggingDrop;

impl EventSink for LoggingDrop {
    fn emit(&self, _event: ProjectedEvent<'_>) {}
}

impl Drop for LoggingDrop {
    fn drop(&mut self) {
        logwise::event!("integration.sinks.destructor", value = local(1_u8));
    }
}

#[derive(Default)]
struct Gate {
    started: Mutex<bool>,
    release: Mutex<bool>,
    changed: Condvar,
}

#[derive(Default)]
struct BlockingWriter {
    gate: Arc<Gate>,
    events: Arc<Mutex<Vec<&'static str>>>,
    first: bool,
    flushes: Arc<AtomicUsize>,
}

impl OwnedEventWriter for BlockingWriter {
    fn write_event(&mut self, event: &OwnedProjectedEvent) -> io::Result<()> {
        if !self.first {
            self.first = true;
            *self.gate.started.lock().unwrap() = true;
            self.gate.changed.notify_all();
            let mut release = self.gate.release.lock().unwrap();
            while !*release {
                release = self.gate.changed.wait(release).unwrap();
            }
        }
        self.events.lock().unwrap().push(event.metadata.event_name);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

fn assert_future(_: &impl core::future::Future<Output = Result<(), logwise_runtime::FlushError>>) {}

fn yield_now() {
    #[cfg(target_arch = "wasm32")]
    wasm_lite_std::yield_now();
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::yield_now();
}

#[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test(worker))]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn bounded_sinks_flush_and_isolate_user_code() {
    #[cfg(not(target_arch = "wasm32"))]
    let previous_panic_hook = {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        previous
    };
    let runtime = logwise_runtime::init().expect("install runtime");

    let memory = Arc::new(InMemorySink::new(2, 4, OverflowPolicy::OverwriteOldest));
    let memory_id = runtime.add_local_sink(
        memory.clone(),
        Filter::new().event("integration.sinks.memory"),
        DetailLevel::Core,
    );
    for value in ["abcdef", "ghijkl", "mnopqr"] {
        logwise::event!("integration.sinks.memory", value = local(value));
    }
    let memory_stats = memory.stats();
    assert_eq!(memory_stats.accepted, 3);
    assert_eq!(memory_stats.overwritten, 1);
    assert_eq!(memory_stats.truncated, 3);
    let retained = memory.drain();
    assert_eq!(retained.len(), 2);
    assert_eq!(
        retained[0].fields[0].value,
        logwise_runtime::OwnedValue::String("ghij".into())
    );
    let rendered = Arc::new(Mutex::new(Vec::new()));
    let mut structured = StructuredWriter::new(SharedBytes(rendered.clone()));
    structured.write_event(&retained[0]).unwrap();
    structured.flush().unwrap();
    assert!(
        String::from_utf8(rendered.lock().unwrap().clone())
            .unwrap()
            .contains("integration.sinks.memory")
    );
    assert!(runtime.remove_sink(memory_id));

    let gate = Arc::new(Gate::default());
    let written = Arc::new(Mutex::new(Vec::new()));
    let flushes = Arc::new(AtomicUsize::new(0));
    let async_sink = AsyncSink::new(
        BlockingWriter {
            gate: gate.clone(),
            events: written.clone(),
            first: false,
            flushes: flushes.clone(),
        },
        1,
        64,
        OverflowPolicy::DropNewest,
    );
    let async_id = runtime.add_local_sink(
        Arc::new(async_sink.clone()),
        Filter::new().event("integration.sinks.async"),
        DetailLevel::Core,
    );

    while async_sink.stats().accepted == 0 {
        logwise::event!("integration.sinks.async.first", value = local(1_u8));
        yield_now();
    }
    let startup_dropped = async_sink.stats().dropped;
    let mut started = gate.started.lock().unwrap();
    while !*started {
        started = gate.changed.wait(started).unwrap();
    }
    drop(started);
    logwise::event!("integration.sinks.async.second", value = local(2_u8));
    logwise::event!("integration.sinks.async.dropped", value = local(3_u8));
    assert_eq!(async_sink.stats().dropped, startup_dropped + 1);

    let barrier = async_sink.flush();
    assert_future(&barrier);
    *gate.release.lock().unwrap() = true;
    gate.changed.notify_all();
    barrier.wait().expect("flush barrier");
    assert_eq!(written.lock().unwrap().len(), 2);
    assert_eq!(flushes.load(Ordering::Relaxed), 1);
    assert_eq!(async_sink.stats().accepted, 2);
    async_sink.emergency_drain().expect("emergency drain");
    assert!(runtime.remove_sink(async_id));
    drop(async_sink);

    #[cfg(not(target_arch = "wasm32"))]
    {
        let panic_writer = AsyncSink::new(PanicWriter, 1, 64, OverflowPolicy::DropNewest);
        let writer_id = runtime.add_local_sink(
            Arc::new(panic_writer.clone()),
            Filter::new().event("integration.sinks.writer_panic"),
            DetailLevel::Core,
        );
        logwise::event!("integration.sinks.writer_panic", value = local(1_u8));
        assert!(panic_writer.flush_blocking().is_err());
        assert!(runtime.remove_sink(writer_id));
        drop(panic_writer);
    }

    let count = Arc::new(CountSink::default());
    let count_id = runtime.add_local_sink(count.clone(), Filter::new(), DetailLevel::Core);
    #[cfg(not(target_arch = "wasm32"))]
    let panic_id = runtime.add_local_sink(Arc::new(PanicSink), Filter::new(), DetailLevel::Core);
    let reentrant_id =
        runtime.add_local_sink(Arc::new(ReentrantSink), Filter::new(), DetailLevel::Core);
    logwise::event!("integration.sinks.outer", value = local(1_u8));
    assert_eq!(count.0.load(Ordering::Relaxed), 1);
    #[cfg(not(target_arch = "wasm32"))]
    assert_eq!(runtime.delivery_stats().sink_panics, 1);
    #[cfg(target_arch = "wasm32")]
    assert_eq!(runtime.delivery_stats().sink_panics, 0);
    assert_eq!(runtime.delivery_stats().reentrant_events_dropped, 1);
    #[cfg(not(target_arch = "wasm32"))]
    assert!(runtime.remove_sink(panic_id));
    assert!(runtime.remove_sink(reentrant_id));

    let drop_id = runtime.add_local_sink(Arc::new(LoggingDrop), Filter::new(), DetailLevel::Core);
    assert!(runtime.remove_sink(drop_id));
    assert_eq!(count.0.load(Ordering::Relaxed), 2);
    assert!(runtime.remove_sink(count_id));

    #[cfg(not(target_arch = "wasm32"))]
    std::panic::set_hook(previous_panic_hook);
}
