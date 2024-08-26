use std::fmt::{Display, Formatter};
use std::io::Write;
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
    type LogRecord = LogRecord;

    fn new_log_record(&self) -> Self::LogRecord {
        LogRecord {
            parts: vec![],
        }
    }

    fn finish_log_record(&self, record: Self::LogRecord) {
        let mut lock = std::io::stderr().lock();
        for part in record.parts {
            lock.write_all(part.as_bytes()).expect("Can't log to stderr");
        }
        lock.write_all(b"\n").expect("Can't log to stderr");
    }

    async fn new_log_record_async(&self) -> Self::LogRecord {
        self.new_log_record()
    }

    async fn finish_log_record_async(&self, record: Self::LogRecord) {
        //todo: this locks, which is maybe not what we want?
        self.finish_log_record(record)
    }

    fn prepare_to_die(&self) {
        //nothing to do since we are unbuffered
    }
}

#[derive(Debug)]
pub struct LogRecord {
    parts: Vec<String>,
}


impl Clone for LogRecord {
    fn clone(&self) -> Self {
        LogRecord {
            parts: self.parts.clone(),
        }
    }
}

impl Display for LogRecord {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for part in &self.parts {
            write!(f, "{}", part)?;
        }
        Ok(())
    }
}

impl From<StdErrorLogger> for LogRecord {
    fn from(value: StdErrorLogger) -> Self {
        LogRecord {
            parts: vec![],
        }
    }
}



impl Into<String> for LogRecord {
    fn into(self) -> String {
        self.parts.join("")
    }
}

impl crate::logger::LogRecord for LogRecord {
    type Logger = StdErrorLogger;

    fn log(&mut self, message: &str) {
        self.parts.push(message.to_string());
    }

    fn log_owned(&mut self, message: String) {
        self.parts.push(message);
    }

}

