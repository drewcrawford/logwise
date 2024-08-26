use crate::log_record::LogRecord;
use crate::privacy::Loggable;



pub struct PrivateFormatter<'a> {
    record: &'a mut LogRecord,
}

impl<'a> PrivateFormatter<'a> {
    pub fn new(record: &'a mut LogRecord) -> Self {
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
```
*/

#[macro_export]
macro_rules! warn_sync {
    //pass to lformat!
    ($($arg:tt)*) => {
        use $crate::hidden::{Logger,LogRecord};
        let mut record = LogRecord::new();
        record.log("WARN: ");
        let mut formatter = dlog::hidden::PrivateFormatter::new(&mut record);
        let global_logger = &dlog::hidden::GLOBAL_LOGGER;

        dlog_proc::lformat!(formatter,$($arg)*);
        global_logger.finish_log_record(record);
    };
}