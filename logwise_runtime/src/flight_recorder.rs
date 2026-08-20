// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded structured history for post-failure and live agent queries.

use std::collections::VecDeque;
use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use logwise::Privacy;

use crate::{EventSink, OwnedProjectedEvent, ProjectedEvent};

/// The privacy projection applied while querying retained history.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RecorderView {
    /// A trusted local view containing support-safe and local-only fields.
    #[default]
    Local,
    /// A support-safe view suitable for serialization to a remote peer.
    Remote,
}

/// A monotonically increasing position in the recorder stream.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct FlightCursor(pub u64);

/// One retained structured event. Formatting is deferred until this value is
/// displayed by a query consumer.
#[derive(Clone, Debug)]
pub struct FlightRecord {
    pub sequence: u64,
    pub event: OwnedProjectedEvent,
}

impl fmt::Display for FlightRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} {:?} {}",
            self.sequence, self.event.metadata.severity, self.event.metadata.event_name
        )?;
        if let Some(message) = &self.event.message {
            write!(formatter, " {message}")?;
        }
        for field in &self.event.fields {
            write!(formatter, " {}={:?}", field.name, field.value)?;
        }
        if self.event.omitted_fields != 0 {
            write!(formatter, " omitted={}", self.event.omitted_fields)?;
        }
        if self.event.truncated_fields != 0 {
            write!(formatter, " truncated={}", self.event.truncated_fields)?;
        }
        Ok(())
    }
}

/// Cumulative recorder accounting. Drops and overwrites are kept separate so
/// a query can distinguish lock contention from bounded-retention loss.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FlightRecorderStats {
    pub accepted: u64,
    pub dropped: u64,
    pub overwritten: u64,
    pub truncated_fields: u64,
}

#[derive(Default)]
struct Stats {
    accepted: AtomicU64,
    dropped: AtomicU64,
    overwritten: AtomicU64,
    truncated_fields: AtomicU64,
}

impl Stats {
    fn snapshot(&self) -> FlightRecorderStats {
        FlightRecorderStats {
            accepted: self.accepted.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
            overwritten: self.overwritten.load(Ordering::Relaxed),
            truncated_fields: self.truncated_fields.load(Ordering::Relaxed),
        }
    }
}

struct Shard {
    capacity: usize,
    records: VecDeque<FlightRecord>,
}

/// The result of a nonblocking recorder read.
///
/// If `busy_shards` is nonzero, `next_cursor` deliberately remains equal to
/// `requested_cursor`. A caller may retry without silently stepping over a
/// record that was temporarily unavailable. Returned records are ordered and
/// may therefore repeat across partial reads; sequence numbers make deduping
/// straightforward.
#[derive(Clone, Debug)]
pub struct FlightRead {
    pub requested_cursor: FlightCursor,
    pub next_cursor: FlightCursor,
    pub records: Vec<FlightRecord>,
    pub dropped_total: u64,
    pub overwritten_total: u64,
    pub truncated_fields_total: u64,
    pub omitted_fields: usize,
    pub busy_shards: usize,
}

impl FlightRead {
    pub const fn is_complete(&self) -> bool {
        self.busy_shards == 0
    }
}

/// A fixed-slot, sharded structured event recorder.
///
/// The recorder is intended to be registered as a local sink with
/// `DetailLevel::Core`. Writes and reads use `try_lock`: a stalled writer or
/// query can make one shard temporarily unavailable, but cannot block the
/// application or prevent other shards from being retrieved.
pub struct FlightRecorder {
    max_string_bytes: usize,
    next_sequence: AtomicU64,
    shards: Vec<Mutex<Shard>>,
    stats: Stats,
}

impl FlightRecorder {
    /// Creates a recorder using a practical platform-derived shard count.
    pub fn new(capacity: usize, max_string_bytes: usize) -> Self {
        let parallelism = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1);
        Self::with_shards(capacity, max_string_bytes, parallelism.min(16))
    }

    /// Creates a recorder with an explicit shard count, primarily for hosts
    /// and deterministic tests. Total retention never exceeds `capacity`.
    pub fn with_shards(capacity: usize, max_string_bytes: usize, shards: usize) -> Self {
        let shard_count = if capacity == 0 {
            1
        } else {
            shards.max(1).min(capacity)
        };
        let base = capacity / shard_count;
        let remainder = capacity % shard_count;
        let shards = (0..shard_count)
            .map(|index| {
                let capacity = base + usize::from(index < remainder);
                Mutex::new(Shard {
                    capacity,
                    records: VecDeque::with_capacity(capacity),
                })
            })
            .collect();
        Self {
            max_string_bytes,
            next_sequence: AtomicU64::new(0),
            shards,
            stats: Stats::default(),
        }
    }

    pub fn stats(&self) -> FlightRecorderStats {
        self.stats.snapshot()
    }

    /// Reads records newer than `cursor` without waiting for a contended shard.
    /// Platform transports can poll this method to mirror history even if a
    /// scheduler thread is stalled.
    pub fn read_since(&self, cursor: FlightCursor, view: RecorderView) -> FlightRead {
        let watermark = self.next_sequence.load(Ordering::Acquire);
        let mut records = Vec::new();
        let mut busy_shards = 0;
        for shard in &self.shards {
            let Ok(shard) = shard.try_lock() else {
                busy_shards += 1;
                continue;
            };
            records.extend(
                shard
                    .records
                    .iter()
                    .filter(|record| record.sequence > cursor.0 && record.sequence <= watermark)
                    .map(|record| project_record(record, view)),
            );
        }
        records.sort_unstable_by_key(|record| record.sequence);
        let stats = self.stats();
        let omitted_fields = records
            .iter()
            .map(|record| record.event.omitted_fields)
            .sum();
        FlightRead {
            requested_cursor: cursor,
            next_cursor: if busy_shards == 0 {
                FlightCursor(watermark.max(cursor.0))
            } else {
                cursor
            },
            records,
            dropped_total: stats.dropped,
            overwritten_total: stats.overwritten,
            truncated_fields_total: stats.truncated_fields,
            omitted_fields,
            busy_shards,
        }
    }

    /// Returns at most the newest `limit` retained records.
    pub fn tail(&self, limit: usize, view: RecorderView) -> FlightRead {
        let mut read = self.read_since(FlightCursor::default(), view);
        if read.records.len() > limit {
            read.records.drain(..read.records.len() - limit);
        }
        read
    }

    fn shard_index(&self) -> usize {
        let mut hasher = DefaultHasher::new();
        std::thread::current().id().hash(&mut hasher);
        (hasher.finish() as usize) % self.shards.len()
    }
}

impl EventSink for FlightRecorder {
    fn emit(&self, mut event: ProjectedEvent<'_>) {
        // Defense in depth: a recorder never persists secrets even when it is
        // invoked directly instead of through Runtime::add_local_sink.
        let before = event.fields.len();
        event
            .fields
            .retain(|field| field.privacy != Privacy::Secret);
        event.omitted_fields += before - event.fields.len();

        let owned = OwnedProjectedEvent::copy_from(event, self.max_string_bytes);
        let index = self.shard_index();
        let Ok(mut shard) = self.shards[index].try_lock() else {
            self.stats.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        if shard.capacity == 0 {
            self.stats.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        if shard.records.len() == shard.capacity {
            shard.records.pop_front();
            self.stats.overwritten.fetch_add(1, Ordering::Relaxed);
        }
        let sequence = self.next_sequence.fetch_add(1, Ordering::AcqRel) + 1;
        self.stats
            .truncated_fields
            .fetch_add(owned.truncated_fields as u64, Ordering::Relaxed);
        shard.records.push_back(FlightRecord {
            sequence,
            event: owned,
        });
        self.stats.accepted.fetch_add(1, Ordering::Relaxed);
    }
}

fn project_record(record: &FlightRecord, view: RecorderView) -> FlightRecord {
    let mut event = record.event.clone();
    // Secret is excluded again in case a future construction path bypasses
    // the sink's ingestion guard.
    let before = event.fields.len();
    event.fields.retain(|field| {
        field.privacy != Privacy::Secret
            && (view == RecorderView::Local || field.privacy == Privacy::SupportSafe)
    });
    event.omitted_fields += before - event.fields.len();
    if view == RecorderView::Remote {
        event.message = None;
    }
    FlightRecord {
        sequence: record.sequence,
        event,
    }
}
