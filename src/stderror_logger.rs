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
        let mut lock = std::io::stderr().lock();
        for part in record.parts {
            lock.write_all(part.as_bytes()).expect("Can't log to stderr");
        }
        lock.write_all(b"\n").expect("Can't log to stderr");
    }

    async fn finish_log_record_async(&self, record: LogRecord) {
        //todo: this locks, which is maybe not what we want?
        self.finish_log_record(record)
    }

    fn prepare_to_die(&self) {
        //nothing to do since we are unbuffered
    }
}
