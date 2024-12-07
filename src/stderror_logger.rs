//SPDX-License-Identifier: MIT OR Apache-2.0
use std::io::Write;
use crate::log_record::LogRecord;
use crate::logger::Logger;

/**
A reference logger that logs to stderr.
 */
#[derive(Debug)]
pub struct StdErrorLogger {}

impl StdErrorLogger {
    pub const fn new() -> Self {
        Self {}
    }
}

impl Logger for StdErrorLogger {

    fn finish_log_record(&self, record: LogRecord) {
        #[cfg(not(target_arch="wasm32"))] {
            let mut lock = std::io::stderr().lock();
            for part in record.parts {
                lock.write_all(part.as_bytes()).expect("Can't log to stderr");
            }
            lock.write_all(b"\n").expect("Can't log to stderr");
        }
        #[cfg(target_arch="wasm32")] {
            let msg = record.parts.join("");
            match record.level() {
                Level::Trace => {
                    web_sys::console::trace_1(&msg.into());
                }
                Level::DebugInternal => {
                    web_sys::console::debug_1(&msg.into());
                }
                Level::Info => {
                    web_sys::console::info_1(&msg.into());
                }
                Level::Analytics => {
                    web_sys::console::info_1(&msg.into());
                }
                Level::PerfWarn => {
                    web_sys::console::warn_1(&msg.into());
                }
                Level::Warning => {
                    web_sys::console::warn_1(&msg.into());
                }
                Level::Error => {
                    web_sys::console::error_1(&msg.into());
                }
                Level::Panic => {
                    web_sys::console::error_1(&msg.into());
                }
            }
        }

    }

    async fn finish_log_record_async(&self, record: LogRecord) {
        //todo: this locks, which is maybe not what we want?
        self.finish_log_record(record)
    }

    fn prepare_to_die(&self) {
        //nothing to do since we are unbuffered
    }
}
