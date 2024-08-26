use crate::context::Context;
use crate::log_record::LogRecord;
use crate::privacy::Loggable;



pub struct PrivateFormatter<'a> {
    record: &'a mut LogRecord,
}

impl<'a> PrivateFormatter<'a> {
    #[inline]
    pub fn new(record: &'a mut LogRecord) -> Self {
        Self { record }
    }
    #[inline]
    pub fn write_literal(&mut self, s: &str) {
        self.record.log(s);
    }
    #[inline]
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
        unsafe {
            use $crate::hidden::Logger;
            let read_ctx = match (&*$crate::context::Context::current()).as_ref() {
                None => {
                    /*
                    Create an additional warning about the missing context.
                     */
                     let mut record = $crate::hidden::LogRecord::new();
                    record.log("WARN: ");

                    //file, line
                    record.log(file!());
                    record.log_owned(format!(":{}:{} ",line!(),column!()));

                    //for warn, we can afford timestamp
                    record.log_timestamp();
                    record.log("No context found. Creating orphan context.");

                    let global_logger = &$crate::hidden::GLOBAL_LOGGER;
                    global_logger.finish_log_record(record);
                    let new_ctx = $crate::context::Context::new_orphan();
                    new_ctx.set_current();
                    (&*$crate::context::Context::current()).as_ref().unwrap()
                }
                Some(ctx) => {
                    ctx
                }

            };

            let mut record = $crate::hidden::LogRecord::new();
            //nest to appropriate level
            for _ in 0..read_ctx.nesting_level() {
                record.log(" ");
            }

            record.log("WARN: ");

            //file, line
            record.log(file!());
            record.log_owned(format!(":{}:{} ",line!(),column!()));

            //for warn, we can afford timestamp
            record.log_timestamp();

            let mut formatter = $crate::hidden::PrivateFormatter::new(&mut record);

            $crate::hidden::lformat!(formatter,$($arg)*);
            //warn sent to global logger
            let global_logger = &$crate::hidden::GLOBAL_LOGGER;
            global_logger.finish_log_record(record);

        }

    };
}


#[cfg(test)] mod tests {
    #[test]
    fn test_warn_sync() {
        crate::context::Context::reset();
        warn_sync!("Hello {world}!",world=23);
    }

}