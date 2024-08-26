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
            let read_ctx = $crate::context::Context::_log_current_context(file!(),line!(),column!());

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
            let read_ctx = $crate::context::Context::_log_current_context(file!(),line!(),column!());

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

/**
Logs a performance warning interval.

```
use dlog::perfwarn;
let _: i32 = perfwarn!("Interval name", {
 //code to profile
    23
});
```
*/
#[macro_export]
macro_rules! perfwarn {
    ($name:literal, $code:block) => {
        {

            let interval = dlog::perfwarn_begin!($name);
            let result = $code;
            drop(interval);
            result
        }
    };
}

#[cfg(test)] mod tests {
    use dlog::context::Context;

    #[test]
    fn test_warn_sync() {
        crate::context::Context::reset();
        warn_sync!("test_warn_sync Hello {world}!",world=23);
    }

    #[test]
    fn test_perfwarn_begin() {
        crate::context::Context::reset();
        let t = perfwarn_begin!("Hello world!");
        warn_sync!("During the test_perfwarn_begin interval");
        drop(t);
    }


    #[test] fn perfwarn() {
        use dlog::perfwarn;
        Context::reset();
        let _: i32 = perfwarn!("test_perfwarn interval name", {
         //code to profile
            23
        });

    }
}