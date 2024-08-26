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
        use $crate::hidden::Logger;
        let mut record = $crate::hidden::LogRecord::new();
        record.log("WARN: ");
        //for warn, we can afford timestamp
        record.log_timestamp();
        let mut formatter = $crate::hidden::PrivateFormatter::new(&mut record);

        $crate::hidden::lformat!(formatter,$($arg)*);
        //warn sent to global logger
        let global_logger = &$crate::hidden::GLOBAL_LOGGER;
        global_logger.finish_log_record(record);
    };
}

#[cfg(test)] mod tests {
    use super::*;
    #[test]
    fn test_warn_sync() {
        let mut record = LogRecord::new();
        let mut formatter = PrivateFormatter::new(&mut record);
        warn_sync!("Hello {world}!",world=23);
    }
}