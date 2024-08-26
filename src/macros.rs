use crate::logger::LogRecord;
use crate::privacy::Loggable;

pub struct _WarnLogger;
impl _WarnLogger {
    pub fn write_literal(&self, s: &str) {
        println!("{}",s);
    }
    pub fn write_val(&self, s: u8) {
        println!("{}",s);
    }
}

pub static _WARN_LOGGER: _WarnLogger = _WarnLogger;

pub struct PrivateFormatter<'a, Record: LogRecord> {
    record: &'a mut Record,
}

impl<'a, Record: LogRecord> PrivateFormatter<'a, Record> {
    pub fn new(record: &'a mut Record) -> Self {
        Self { record }
    }
    pub fn write_literal(&mut self, s: &str) {
        self.record.log(s);
    }
    pub fn write_val<Val: Loggable>(&mut self, s: Val) {
        s.log_all(self.record);
    }
}

/**
Logs a message at warning level.

```
use dlog::warn_sync;
warn_sync!("Hello {world}!",world=23);
todo!();
```
*/

#[macro_export]
macro_rules! warn_sync {
    //pass to lformat!
    ($($arg:tt)*) => {
        use $crate::hidden::{Logger,LogRecord};
        let global_logger = &dlog::hidden::GLOBAL_LOGGER;
        let mut record = global_logger.new_log_record();
        record.log("WARN: ");
        let mut formatter = dlog::hidden::PrivateFormatter::new(&mut record);
        dlog_proc::lformat!(formatter,$($arg)*);
        global_logger.finish_log_record(record);
    };
}