use crate::log_record::LogRecord;
use crate::privacy::Loggable;

//macros that got ported to proc
pub use dlog_proc::debuginternal_sync;

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

pub fn debuginternal_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    //safety: guarantee context won't change
    let mut record = crate::hidden::LogRecord::new();

    unsafe {
        let read_ctx = crate::context::Context::_log_current_context(file,line,column);
        read_ctx._log_prelude(&mut record);

    }


    record.log("INFO: ");

    //file, line
    record.log(file!());
    record.log_owned(format!(":{}:{} ",line,column));

    //for info, we can afford timestamp
    record.log_timestamp();

    record
}

pub fn debuginternal_sync_post(record: LogRecord) {
    use crate::logger::Logger;
    let global_logger = &crate::hidden::GLOBAL_LOGGER;
    global_logger.finish_log_record(record);
}

pub async fn debuginternal_async_post(record: LogRecord) {
    use crate::logger::Logger;
    let global_logger = &crate::hidden::GLOBAL_LOGGER;
    global_logger.finish_log_record_async(record).await;
}



pub fn info_sync_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    //safety: guarantee context won't change
    let mut record = crate::hidden::LogRecord::new();

    unsafe {
        let read_ctx = crate::context::Context::_log_current_context(file, line, column);
        read_ctx._log_prelude(&mut record);
    }

    record.log("INFO: ");

    //file, line
    record.log(file!());
    record.log_owned(format!(":{}:{} ", line!(), column!()));

    //for info, we can afford timestamp
    record.log_timestamp();
    record
}

pub fn info_sync_post(record: LogRecord) {
    use crate::logger::Logger;
    let global_logger = &crate::hidden::GLOBAL_LOGGER;
    global_logger.finish_log_record(record);
}


pub fn warn_sync_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    //safety: guarantee context won't change
    let mut record = crate::hidden::LogRecord::new();

    unsafe {
        let read_ctx = crate::context::Context::_log_current_context(file, line, column);
        read_ctx._log_prelude(&mut record);
    }

    record.log("WARN: ");

    //file, line
    record.log(file!());
    record.log_owned(format!(":{}:{} ", line!(), column!()));

    //for warn, we can afford timestamp
    record.log_timestamp();
    record
}

pub fn warn_sync_post(record: LogRecord) {
    use crate::logger::Logger;
    let global_logger = &crate::hidden::GLOBAL_LOGGER;
    global_logger.finish_log_record(record);
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
        {
            let mut record = $crate::hidden::warn_sync_pre(file!(),line!(),column!());

            let mut formatter = $crate::hidden::PrivateFormatter::new(&mut record);

            $crate::hidden::lformat!(formatter,$($arg)*);
            $crate::hidden::warn_sync_post(record);
        }

    };
}

pub fn perfwarn_begin_pre(file: &'static str, line: u32, column: u32, name: &'static str) -> LogRecord {
    let start = std::time::Instant::now();

    //safety: guarantee context won't change
    let mut record = crate::hidden::LogRecord::new();

    unsafe {
        let read_ctx = crate::context::Context::_log_current_context(file, line, column);
        read_ctx._log_prelude(&mut record);
    }

    record.log("PERFWARN: BEGIN ");

    //file, line
    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    record.log(name);
    record.log(" ");
    record.log_time_since(start);
    record.log(name);
    record
}

pub fn perfwarn_begin_post(record: LogRecord) -> crate::interval::PerfwarnInterval {
    use crate::logger::Logger;
    let global_logger = &crate::hidden::GLOBAL_LOGGER;
    global_logger.finish_log_record(record);
    let interval = crate::interval::PerfwarnInterval::new("Interval name", std::time::Instant::now());
    interval
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
        {
            let record = $crate::hidden::perfwarn_begin_pre(file!(),line!(),column!(),$name);

            $crate::hidden::perfwarn_begin_post(record)
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
    use dlog::hidden::PrivateFormatter;
    use dlog::warn_sync;
    use dlog::hidden::info_sync;
    use dlog_proc::debuginternal_sync;

    #[test]
    fn test_warn_sync() {
        crate::context::Context::reset();
        info_sync!("test_warn_sync");
        warn_sync!("test_warn_sync Hello {world}!",world=23);
    }

    #[test]
    fn test_perfwarn_begin() {
        crate::context::Context::reset();
        info_sync!("test_perfwarn_begin");
        let t = perfwarn_begin!("Hello world!");
        warn_sync!("During the test_perfwarn_begin interval");
        drop(t);
    }


    #[test] fn perfwarn() {
        use dlog::perfwarn;
        Context::reset();
        info_sync!("test_perfwarn");
        let _: i32 = perfwarn!("test_perfwarn interval name", {
         //code to profile
            23
        });

    }
    #[test] fn test_debuginternal_sync() {
        crate::context::Context::reset();
        debuginternal_sync!("test_debuginternal_sync");
    }
    #[test] fn test_log_rich() {
        let val = false;
        crate::context::Context::reset();
        let mut record = crate::log_record::LogRecord::new();
        let mut formatter = PrivateFormatter::new(&mut record);
        crate::hidden::lformat!(formatter,"Hello {world}!",world=val);

        // debuginternal_sync!("Hello {world}!",world=val);

    }
}