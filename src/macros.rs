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



pub fn perfwarn_begin_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
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

    record.log_time_since(start);
    record
}

pub fn perfwarn_begin_post(record: LogRecord,name: &'static str) -> crate::interval::PerfwarnInterval {
    use crate::logger::Logger;
    let global_logger = &crate::hidden::GLOBAL_LOGGER;
    global_logger.finish_log_record(record);
    let interval = crate::interval::PerfwarnInterval::new(name, std::time::Instant::now());
    interval
}



#[cfg(test)] mod tests {
    use dlog::context::Context;
    use dlog_proc::{debuginternal_sync, info_sync, warn_sync};

    #[test]
    fn test_warn_sync() {
        crate::context::Context::reset();
        info_sync!("test_warn_sync");
        warn_sync!("test_warn_sync Hello {world}!",world=23);
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

        debuginternal_sync!("Hello {world}!",world=val);

    }
}