// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reference [`Logger`] implementation, and the one most applications install: a
//! zero-sized sink that writes finished records to stderr.
//!
//! It reports [`LogPrivacy::Private`], so records reach it unredacted -- appropriate for
//! a developer's terminal, not for a shipped telemetry sink. Off wasm it takes the stderr
//! lock once per record, which is what keeps concurrently logged lines from interleaving;
//! on wasm32 there is no stderr, so the parts are joined and routed to the `console`
//! method matching the record's [`Level`](crate::Level). Output is unbuffered, so
//! `prepare_to_die` has nothing to flush and the async path defers to the blocking one.

use crate::log_record::LogRecord;
use crate::logger::{LogPrivacy, Logger};

/**
A reference logger that logs to stderr.
 */
#[derive(Debug, Clone)]
pub struct StdErrorLogger {}

// ============================================================================
// BOILERPLATE TRAIT IMPLEMENTATIONS
// ============================================================================
//
// Design decisions for StdErrorLogger trait implementations:
//
// - Debug/Clone: Already derived - appropriate for zero-sized struct
// - Copy: Implemented - safe for zero-sized struct with no heap allocation
// - PartialEq/Eq: Implemented - all instances are equivalent (zero-sized)
// - Hash: Implemented - consistent with Eq, enables use as hash map keys
// - Default: Implemented - provides convenient zero-argument constructor
// - Display: NOT implemented - no meaningful string representation for stderr logger
// - From/Into: NOT implemented - no obvious conversions
// - Send/Sync: Automatically implemented - zero-sized struct is always thread-safe

impl Copy for StdErrorLogger {}

impl PartialEq for StdErrorLogger {
    fn eq(&self, _other: &Self) -> bool {
        // All instances of a zero-sized struct are equal
        true
    }
}

impl Eq for StdErrorLogger {}

impl std::hash::Hash for StdErrorLogger {
    fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {
        // Zero-sized struct has no data to hash - this is consistent with Eq
    }
}

impl Default for StdErrorLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl StdErrorLogger {
    pub const fn new() -> Self {
        Self {}
    }
}

impl Logger for StdErrorLogger {
    fn privacy(&self) -> LogPrivacy {
        LogPrivacy::Private
    }

    fn finish_log_record(&self, record: LogRecord) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::io::Write;
            let mut lock = std::io::stderr().lock();
            for part in record.parts {
                lock.write_all(part.as_bytes())
                    .expect("Can't log to stderr");
            }
            lock.write_all(b"\n").expect("Can't log to stderr");
        }
        #[cfg(target_arch = "wasm32")]
        {
            use crate::Level;
            let msg = record.parts.join("");
            match record.level() {
                Level::Trace => {
                    wasm_lite::console::trace(&msg);
                }
                Level::DebugInternal => {
                    wasm_lite::console::debug(&msg);
                }
                Level::Info => {
                    wasm_lite::console::info(&msg);
                }
                Level::Analytics => {
                    wasm_lite::console::info(&msg);
                }
                Level::PerfWarn => {
                    wasm_lite::console::warn(&msg);
                }
                Level::Warning => {
                    wasm_lite::console::warn(&msg);
                }
                Level::Error => {
                    wasm_lite::console::error(&msg);
                }
                Level::Panic => {
                    wasm_lite::console::error(&msg);
                }
                Level::Mandatory => {
                    wasm_lite::console::log(&msg);
                }
                Level::Profile => {
                    wasm_lite::console::log(&msg);
                }
            }
        }
    }

    fn finish_log_record_async<'s>(
        &'s self,
        record: LogRecord,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 's>> {
        Box::pin(async move {
            //todo: this locks, which is maybe not what we want?
            self.finish_log_record(record)
        })
    }

    fn prepare_to_die(&self) {
        //nothing to do since we are unbuffered
    }
}
