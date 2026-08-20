// SPDX-License-Identifier: MIT OR Apache-2.0

//! The shipped destinations, and what each one does when it cannot keep up.
//!
//! Console, bounded in-memory, structured-writer and queued async sinks all
//! sit behind the runtime dispatcher and receive
//! [`ProjectedEvent`], never raw facade values. Every
//! buffer here is bounded, and every one accounts for what it lost: accepted,
//! dropped, overwritten, truncated and failed records are counted rather than
//! silently discarded, because a sink that quietly drops is indistinguishable
//! from a program that never logged.
//!
//! Two rules hold across all of them. A write that fails is dropped rather
//! than panicked on — stderr breaks for reasons unrelated to the code being
//! diagnosed, and these are called from destructors where a panic aborts. And
//! sink callbacks run outside the configuration locks, so a slow or hostile
//! sink cannot stall the call site or poison a lock for every sink after it.

use std::collections::VecDeque;
use std::fmt;
use std::future::Future;
use std::io::{self, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Waker};

use logwise::{ContextToken, Detail, Metadata, Privacy, ValueRef};

use crate::{EventSink, ProjectedEvent};

#[derive(Clone, Debug, PartialEq)]
pub enum OwnedValue {
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
    String(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct OwnedField {
    pub name: &'static str,
    pub privacy: Privacy,
    pub detail: Detail,
    pub value: OwnedValue,
}

#[derive(Clone, Debug)]
pub struct OwnedProjectedEvent {
    pub metadata: &'static Metadata,
    pub context: ContextToken,
    pub fields: Vec<OwnedField>,
    pub message: Option<String>,
    pub omitted_fields: usize,
    pub truncated_fields: usize,
}

impl OwnedProjectedEvent {
    pub fn copy_from(event: ProjectedEvent<'_>, max_string_bytes: usize) -> Self {
        let mut truncated_fields = 0;
        let fields = event
            .fields
            .into_iter()
            .map(|field| OwnedField {
                name: field.name,
                privacy: field.privacy,
                detail: field.detail,
                value: own_value(field.value, max_string_bytes, &mut truncated_fields),
            })
            .collect();
        let message = event
            .message
            .map(|message| truncate(message.to_string(), max_string_bytes, &mut truncated_fields));
        Self {
            metadata: event.metadata,
            context: event.context,
            fields,
            message,
            omitted_fields: event.omitted_fields,
            truncated_fields,
        }
    }
}

fn own_value(value: ValueRef<'_>, max: usize, truncated: &mut usize) -> OwnedValue {
    match value {
        ValueRef::Bool(value) => OwnedValue::Bool(value),
        ValueRef::I64(value) => OwnedValue::I64(value),
        ValueRef::U64(value) => OwnedValue::U64(value),
        ValueRef::F64(value) => OwnedValue::F64(value),
        ValueRef::Str(value) => OwnedValue::String(truncate(value.to_owned(), max, truncated)),
        ValueRef::Debug(value) => {
            OwnedValue::String(truncate(format!("{value:?}"), max, truncated))
        }
        ValueRef::Display(value) => {
            OwnedValue::String(truncate(format!("{value}"), max, truncated))
        }
    }
}

fn truncate(mut value: String, max: usize, truncated: &mut usize) -> String {
    if value.len() <= max {
        return value;
    }
    let mut boundary = max;
    while boundary > 0 && !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
    *truncated += 1;
    value
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum OverflowPolicy {
    #[default]
    DropNewest,
    OverwriteOldest,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct SinkStats {
    pub accepted: u64,
    pub dropped: u64,
    pub overwritten: u64,
    pub truncated: u64,
    pub write_errors: u64,
}

#[derive(Default)]
struct Stats {
    accepted: AtomicU64,
    dropped: AtomicU64,
    overwritten: AtomicU64,
    truncated: AtomicU64,
    write_errors: AtomicU64,
}

impl Stats {
    fn snapshot(&self) -> SinkStats {
        SinkStats {
            accepted: self.accepted.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
            overwritten: self.overwritten.load(Ordering::Relaxed),
            truncated: self.truncated.load(Ordering::Relaxed),
            write_errors: self.write_errors.load(Ordering::Relaxed),
        }
    }
}

pub struct InMemorySink {
    capacity: usize,
    max_string_bytes: usize,
    overflow: OverflowPolicy,
    records: Mutex<VecDeque<OwnedProjectedEvent>>,
    stats: Stats,
}

impl fmt::Debug for InMemorySink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `records` is deliberately not locked: formatting must not be able to
        // block, and `stats` answers the question a reader actually has.
        formatter
            .debug_struct("InMemorySink")
            .field("capacity", &self.capacity)
            .field("max_string_bytes", &self.max_string_bytes)
            .field("overflow", &self.overflow)
            .field("stats", &self.stats.snapshot())
            .finish_non_exhaustive()
    }
}

impl InMemorySink {
    pub fn new(capacity: usize, max_string_bytes: usize, overflow: OverflowPolicy) -> Self {
        Self {
            capacity,
            max_string_bytes,
            overflow,
            records: Mutex::new(VecDeque::with_capacity(capacity)),
            stats: Stats::default(),
        }
    }

    pub fn drain(&self) -> Vec<OwnedProjectedEvent> {
        self.records.lock().unwrap().drain(..).collect()
    }

    pub fn stats(&self) -> SinkStats {
        self.stats.snapshot()
    }
}

impl EventSink for InMemorySink {
    fn emit(&self, event: ProjectedEvent<'_>) {
        if self.capacity == 0 {
            self.stats.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let owned = OwnedProjectedEvent::copy_from(event, self.max_string_bytes);
        self.stats
            .truncated
            .fetch_add(owned.truncated_fields as u64, Ordering::Relaxed);
        let Ok(mut records) = self.records.try_lock() else {
            self.stats.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        if records.len() == self.capacity {
            match self.overflow {
                OverflowPolicy::DropNewest => {
                    self.stats.dropped.fetch_add(1, Ordering::Relaxed);
                    return;
                }
                OverflowPolicy::OverwriteOldest => {
                    records.pop_front();
                    self.stats.overwritten.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        records.push_back(owned);
        self.stats.accepted.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Debug, Default)]
pub struct ConsoleSink;

impl EventSink for ConsoleSink {
    fn emit(&self, event: ProjectedEvent<'_>) {
        // `eprintln!` panics when stderr will not take a write, and a sink is
        // the wrong place to learn that the terminal went away: the record is
        // lost either way, but a panic also costs an unwind through the
        // runtime's fan-out on every subsequent event.
        let _ = writeln!(io::stderr().lock(), "{}", RenderedEvent(&event));
    }
}

struct RenderedEvent<'a>(&'a ProjectedEvent<'a>);

impl fmt::Display for RenderedEvent<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let event = self.0;
        write!(
            formatter,
            "{:?} {}",
            event.metadata.severity, event.metadata.event_name
        )?;
        if let Some(message) = event.message {
            write!(formatter, " {message}")?;
        }
        for field in &event.fields {
            write!(formatter, " {}={:?}", field.name, field.value)?;
        }
        if event.omitted_fields != 0 {
            write!(formatter, " omitted={}", event.omitted_fields)?;
        }
        Ok(())
    }
}

pub trait OwnedEventWriter: Send + 'static {
    fn write_event(&mut self, event: &OwnedProjectedEvent) -> io::Result<()>;
    fn flush(&mut self) -> io::Result<()>;
}

pub struct StructuredWriter<W> {
    writer: W,
}

impl<W> fmt::Debug for StructuredWriter<W> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `W` is an arbitrary writer and is not required to be `Debug`.
        formatter
            .debug_struct("StructuredWriter")
            .finish_non_exhaustive()
    }
}

impl<W> StructuredWriter<W> {
    pub const fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write + Send + 'static> OwnedEventWriter for StructuredWriter<W> {
    fn write_event(&mut self, event: &OwnedProjectedEvent) -> io::Result<()> {
        write!(
            self.writer,
            "{:?} {}",
            event.metadata.severity, event.metadata.event_name
        )?;
        if let Some(message) = &event.message {
            write!(self.writer, " {message}")?;
        }
        for field in &event.fields {
            write!(self.writer, " {}={:?}", field.name, field.value)?;
        }
        writeln!(self.writer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

struct QueuedEvent {
    sequence: u64,
    event: OwnedProjectedEvent,
}

#[derive(Default)]
struct Queue {
    events: VecDeque<QueuedEvent>,
    shutdown: bool,
}

struct AsyncShared {
    capacity: usize,
    max_string_bytes: usize,
    overflow: OverflowPolicy,
    queue: Mutex<Queue>,
    work: Condvar,
    progress_lock: Mutex<()>,
    progress: Condvar,
    next_sequence: AtomicU64,
    processed_sequence: AtomicU64,
    requested_flush: AtomicU64,
    flushed_sequence: AtomicU64,
    stopped: AtomicBool,
    wakers: Mutex<Vec<Waker>>,
    last_flush_error: Mutex<Option<(u64, FlushError)>>,
    last_write_error: Mutex<Option<(u64, FlushError)>>,
    stats: Stats,
}

pub struct AsyncSink {
    inner: Arc<AsyncInner>,
}

struct AsyncInner {
    shared: Arc<AsyncShared>,
    worker: Mutex<Option<wasm_lite_std::JoinHandle<()>>>,
}

impl fmt::Debug for AsyncSink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never touches `worker`: that mutex is held across the worker's own
        // shutdown, so formatting under it could park the caller.
        formatter
            .debug_struct("AsyncSink")
            .field(
                "stopped",
                &self.inner.shared.stopped.load(Ordering::Acquire),
            )
            .field(
                "flushed_sequence",
                &self.inner.shared.flushed_sequence.load(Ordering::Acquire),
            )
            .finish_non_exhaustive()
    }
}

impl AsyncSink {
    pub fn new<W: OwnedEventWriter>(
        writer: W,
        capacity: usize,
        max_string_bytes: usize,
        overflow: OverflowPolicy,
    ) -> Self {
        let shared = Arc::new(AsyncShared {
            capacity,
            max_string_bytes,
            overflow,
            queue: Mutex::new(Queue::default()),
            work: Condvar::new(),
            progress_lock: Mutex::new(()),
            progress: Condvar::new(),
            next_sequence: AtomicU64::new(0),
            processed_sequence: AtomicU64::new(0),
            requested_flush: AtomicU64::new(0),
            flushed_sequence: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
            wakers: Mutex::new(Vec::new()),
            last_flush_error: Mutex::new(None),
            last_write_error: Mutex::new(None),
            stats: Stats::default(),
        });
        let worker_shared = shared.clone();
        let panic_shared = worker_shared.clone();
        let worker = wasm_lite_std::spawn(move || {
            if catch_unwind(AssertUnwindSafe(|| worker_loop(worker_shared, writer))).is_err() {
                panic_shared
                    .stats
                    .write_errors
                    .fetch_add(1, Ordering::Relaxed);
                panic_shared.stopped.store(true, Ordering::Release);
                notify_progress(&panic_shared);
            }
        });
        Self {
            inner: Arc::new(AsyncInner {
                shared,
                worker: Mutex::new(Some(worker)),
            }),
        }
    }

    pub fn stats(&self) -> SinkStats {
        self.inner.shared.stats.snapshot()
    }

    pub fn flush(&self) -> FlushBarrier {
        let shared = self.inner.shared.clone();
        let target = {
            let _queue = shared.queue.lock().unwrap();
            shared.next_sequence.load(Ordering::Acquire)
        };
        shared.requested_flush.fetch_max(target, Ordering::AcqRel);
        shared.work.notify_one();
        FlushBarrier { shared, target }
    }

    pub fn flush_blocking(&self) -> Result<(), FlushError> {
        self.flush().wait()
    }

    pub fn emergency_drain(&self) -> Result<(), FlushError> {
        self.flush_blocking()
    }
}

impl Clone for AsyncSink {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl EventSink for AsyncSink {
    fn emit(&self, event: ProjectedEvent<'_>) {
        let shared = &self.inner.shared;
        if shared.capacity == 0 || shared.stopped.load(Ordering::Acquire) {
            shared.stats.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let owned = OwnedProjectedEvent::copy_from(event, shared.max_string_bytes);
        shared
            .stats
            .truncated
            .fetch_add(owned.truncated_fields as u64, Ordering::Relaxed);
        let Ok(mut queue) = shared.queue.try_lock() else {
            shared.stats.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        if queue.events.len() == shared.capacity {
            match shared.overflow {
                OverflowPolicy::DropNewest => {
                    shared.stats.dropped.fetch_add(1, Ordering::Relaxed);
                    return;
                }
                OverflowPolicy::OverwriteOldest => {
                    let requested = shared.requested_flush.load(Ordering::Acquire);
                    let flushed = shared.flushed_sequence.load(Ordering::Acquire);
                    if requested > flushed
                        && queue
                            .events
                            .front()
                            .is_some_and(|event| event.sequence <= requested)
                    {
                        // Once a barrier exists, prior accepted records may no
                        // longer be overwritten out from under its guarantee.
                        shared.stats.dropped.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                    queue.events.pop_front();
                    shared.stats.overwritten.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        let sequence = shared.next_sequence.fetch_add(1, Ordering::AcqRel) + 1;
        queue.events.push_back(QueuedEvent {
            sequence,
            event: owned,
        });
        shared.stats.accepted.fetch_add(1, Ordering::Relaxed);
        drop(queue);
        shared.work.notify_one();
    }
}

impl Drop for AsyncInner {
    fn drop(&mut self) {
        {
            let mut queue = self.shared.queue.lock().unwrap();
            queue.shutdown = true;
        }
        self.shared.requested_flush.store(
            self.shared.next_sequence.load(Ordering::Acquire),
            Ordering::Release,
        );
        self.shared.work.notify_one();
        if let Some(worker) = self.worker.lock().unwrap().take() {
            #[cfg(target_arch = "wasm32")]
            if wasm_lite_std::is_main_thread() {
                // Joining uses a blocking wait, which browser main threads do
                // not support. Dropping detaches while the signalled worker
                // performs its best-effort drain.
                drop(worker);
            } else {
                let _ = worker.join();
            }
            #[cfg(not(target_arch = "wasm32"))]
            let _ = worker.join();
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FlushError {
    pub kind: io::ErrorKind,
    pub message: String,
}

impl From<io::Error> for FlushError {
    fn from(error: io::Error) -> Self {
        Self {
            kind: error.kind(),
            message: error.to_string(),
        }
    }
}

impl fmt::Display for FlushError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for FlushError {}

pub struct FlushBarrier {
    shared: Arc<AsyncShared>,
    target: u64,
}

impl fmt::Debug for FlushBarrier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FlushBarrier")
            .field("target", &self.target)
            .field(
                "flushed_sequence",
                &self.shared.flushed_sequence.load(Ordering::Acquire),
            )
            .finish_non_exhaustive()
    }
}

impl FlushBarrier {
    fn result(&self) -> Option<Result<(), FlushError>> {
        if self.shared.flushed_sequence.load(Ordering::Acquire) < self.target {
            if self.shared.stopped.load(Ordering::Acquire) {
                return Some(Err(FlushError {
                    kind: io::ErrorKind::BrokenPipe,
                    message: "logwise sink worker stopped before the barrier".into(),
                }));
            }
            return None;
        }
        let error = self
            .shared
            .last_flush_error
            .lock()
            .unwrap()
            .clone()
            .filter(|(sequence, _)| *sequence >= self.target)
            .map(|(_, error)| error);
        let write_error = self
            .shared
            .last_write_error
            .lock()
            .unwrap()
            .clone()
            .filter(|(sequence, _)| *sequence <= self.target)
            .map(|(_, error)| error);
        Some(error.or(write_error).map_or(Ok(()), Err))
    }

    pub fn wait(self) -> Result<(), FlushError> {
        let mut progress = self.shared.progress_lock.lock().unwrap();
        loop {
            if let Some(result) = self.result() {
                return result;
            }
            progress = self.shared.progress.wait(progress).unwrap();
        }
    }
}

impl Future for FlushBarrier {
    type Output = Result<(), FlushError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(result) = self.result() {
            return Poll::Ready(result);
        }
        let mut wakers = self.shared.wakers.lock().unwrap();
        if !wakers.iter().any(|waker| waker.will_wake(context.waker())) {
            wakers.push(context.waker().clone());
        }
        drop(wakers);
        self.result().map_or(Poll::Pending, Poll::Ready)
    }
}

fn worker_loop<W: OwnedEventWriter>(shared: Arc<AsyncShared>, mut writer: W) {
    loop {
        let event = {
            let mut queue = shared.queue.lock().unwrap();
            loop {
                if let Some(event) = queue.events.pop_front() {
                    break Some(event);
                }
                let requested = shared.requested_flush.load(Ordering::Acquire);
                let flushed = shared.flushed_sequence.load(Ordering::Acquire);
                if requested > flushed || queue.shutdown {
                    break None;
                }
                queue = shared.work.wait(queue).unwrap();
            }
        };

        if let Some(queued) = event {
            if let Err(error) = writer.write_event(&queued.event) {
                shared.stats.write_errors.fetch_add(1, Ordering::Relaxed);
                let mut last = shared.last_write_error.lock().unwrap();
                if last.is_none() {
                    *last = Some((queued.sequence, FlushError::from(error)));
                }
            }
            shared
                .processed_sequence
                .store(queued.sequence, Ordering::Release);
        }

        let processed = shared.processed_sequence.load(Ordering::Acquire);
        let requested = shared.requested_flush.load(Ordering::Acquire);
        if requested <= processed && requested > shared.flushed_sequence.load(Ordering::Acquire) {
            match writer.flush() {
                Ok(()) => {
                    *shared.last_flush_error.lock().unwrap() = None;
                }
                Err(error) => {
                    shared.stats.write_errors.fetch_add(1, Ordering::Relaxed);
                    *shared.last_flush_error.lock().unwrap() =
                        Some((processed, FlushError::from(error)));
                }
            }
            shared.flushed_sequence.store(processed, Ordering::Release);
            notify_progress(&shared);
        }

        let should_stop = {
            let queue = shared.queue.lock().unwrap();
            queue.shutdown && queue.events.is_empty()
        };
        if should_stop {
            if shared.flushed_sequence.load(Ordering::Acquire) < processed {
                let _ = writer.flush();
                shared.flushed_sequence.store(processed, Ordering::Release);
            }
            shared.stopped.store(true, Ordering::Release);
            notify_progress(&shared);
            return;
        }
    }
}

fn notify_progress(shared: &AsyncShared) {
    {
        // `FlushBarrier::wait` evaluates its predicate while holding this lock and
        // then blocks on the condvar, which releases it. Taking the lock here is
        // what makes that sequence indivisible from the worker's point of view: a
        // barrier that has already read a stale `flushed_sequence` still holds the
        // lock, so this thread cannot signal into the gap before the barrier is
        // parked. Every caller publishes its state change before calling here, so
        // acquiring the lock afterwards is enough — the state is never read under
        // it by this thread.
        let _parked = shared.progress_lock.lock().unwrap_or_else(|error| {
            // A panicking waiter poisons the lock, but the waiter observes state
            // through atomics rather than the guarded value, so there is nothing
            // to distrust here. Failing to notify would hang every other barrier.
            error.into_inner()
        });
        shared.progress.notify_all();
    }
    let wakers = std::mem::take(&mut *shared.wakers.lock().unwrap());
    for waker in wakers {
        waker.wake();
    }
}
