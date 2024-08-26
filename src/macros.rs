use crate::log_record::LogRecord;
use crate::privacy::Loggable;

pub use dlog_proc::perfwarn;

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
            let read_ctx = $crate::context::Context::_log_current_context();

            let mut record = $crate::hidden::LogRecord::new();
            read_ctx._log_prelude(&mut record);

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

/**
Logs a performance warning interval.

Takes a single argument, the interval name.

```
use dlog::perfwarn_begin;
let interval = perfwarn_begin!("Interval name");
drop(interval);
```
*/
#[macro_export]
macro_rules! perfwarn_begin {
    ($name:literal) => {
        unsafe {
            let start = std::time::Instant::now();

            use $crate::hidden::Logger;
            let read_ctx = $crate::context::Context::_log_current_context();

            let mut record = $crate::hidden::LogRecord::new();
            read_ctx._log_prelude(&mut record);

            record.log("PERFWARN: BEGIN ");
            let interval = $crate::interval::PerfwarnInterval::new($name,start);


            interval.log_timestamp(&mut record);
            record.log($name);

            let global_logger = &$crate::hidden::GLOBAL_LOGGER;
            global_logger.finish_log_record(record);

            interval
        }
    };
}


#[cfg(test)] mod tests {
    #[test]
    fn test_warn_sync() {
        crate::context::Context::reset();
        warn_sync!("Hello {world}!",world=23);
    }

    #[test]
    fn test_perfwarn_begin() {
        crate::context::Context::reset();
        let t = perfwarn_begin!("Hello world!");
        warn_sync!("During the interval");
        drop(t);
    }


    #[test] fn perfwarn() {
        crate::context::Context::reset();
        use crate::macros::perfwarn;
        #[perfwarn("perfwarn test")] fn ex() {
            warn_sync!("Hello {world}!",world=23);
        }
    }
}