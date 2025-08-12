//SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::atomic::AtomicBool;
use crate::Level;
use crate::log_record::LogRecord;
use crate::privacy::Loggable;

pub struct LoggingDomain {
    is_internal: AtomicBool,
}

impl LoggingDomain {
    #[inline] pub const fn new(enabled: bool) -> Self {
        Self {
            is_internal: AtomicBool::new(enabled),
        }
    }

    #[inline] pub fn is_internal(&self) -> bool {
        self.is_internal.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[macro_export]
macro_rules! declare_logging_domain {
    () => {
        #[doc(hidden)]
        #[cfg(feature="logwise_internal")]
        pub(crate) static __LOGWISE_DOMAIN: $crate::LoggingDomain = $crate::LoggingDomain::new(true);
        #[cfg(not(feature="logwise_internal"))]
        pub(crate) static __LOGWISE_DOMAIN: $crate::LoggingDomain = $crate::LoggingDomain::new(false);
    };
    ($enabled:expr) => {
        #[doc(hidden)]
        pub static __LOGWISE_DOMAIN: $crate::LoggingDomain = $crate::LoggingDomain::new($enabled);
    };
}





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
    let mut record = crate::log_record::LogRecord::new(Level::DebugInternal);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);


    record.log("DEBUG: ");

    //file, line
    record.log(file);
    record.log_owned(format!(":{}:{} ",line,column));

    //for info, we can afford timestamp
    record.log_timestamp();

    record
}

pub fn debuginternal_sync_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record(record.clone());
    }
}

pub async fn debuginternal_async_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record_async(record.clone()).await;
    }
}



pub fn info_sync_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    //safety: guarantee context won't change
    let mut record = crate::log_record::LogRecord::new(Level::Info);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);


    record.log("INFO: ");

    //file, line
    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    //for info, we can afford timestamp
    record.log_timestamp();
    record
}

pub fn info_sync_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        //can't call eprintln in wasm32!
        // eprintln!("Sending to logger: {:?}", logger);
        logger.finish_log_record(record.clone());
    }
}

pub async fn info_async_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record_async(record.clone()).await;
    }
}


pub fn warn_sync_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    //safety: guarantee context won't change
    let mut record = crate::log_record::LogRecord::new(Level::Warning);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);

    record.log("WARN: ");

    //file, line
    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    //for warn, we can afford timestamp
    record.log_timestamp();
    record
}

pub fn warn_sync_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record(record.clone());
    }
}

pub fn trace_sync_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    //safety: guarantee context won't change
    let mut record = crate::log_record::LogRecord::new(Level::Trace);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);

    record.log("TRACE: ");

    //file, line
    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    //for warn, we can afford timestamp
    record.log_timestamp();
    record
}

pub fn trace_sync_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record(record.clone());
    }
}

pub async fn trace_async_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record_async(record.clone()).await;
    }
}

pub fn error_sync_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    //safety: guarantee context won't change
    let mut record = crate::log_record::LogRecord::new(Level::Error);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);

    record.log("ERROR: ");

    //file, line
    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    //for warn, we can afford timestamp
    record.log_timestamp();
    record
}

pub fn error_sync_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record(record.clone());
    }
}

pub async fn error_async_post(record: LogRecord) {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record_async(record.clone()).await;
    }
}


pub fn perfwarn_begin_pre(file: &'static str, line: u32, column: u32) -> LogRecord {
    let start = crate::sys::Instant::now();

    //safety: guarantee context won't change
    let mut record = crate::log_record::LogRecord::new(Level::PerfWarn);

    let read_ctx = crate::context::Context::current();
    read_ctx._log_prelude(&mut record);

    record.log("PERFWARN: BEGIN ");

    //file, line
    record.log(file);
    record.log_owned(format!(":{}:{} ", line, column));

    record.log_time_since(start);
    record
}

pub fn perfwarn_begin_post(record: LogRecord,name: &'static str) -> crate::interval::PerfwarnInterval {
    let global_loggers = crate::hidden::global_loggers();
    for logger in global_loggers {
        logger.finish_log_record(record.clone());
    }
    
    crate::interval::PerfwarnInterval::new(name, crate::sys::Instant::now())
}



#[cfg(test)] mod tests {
    use logwise::context::Context;
    use logwise_proc::{debuginternal_sync, info_sync, warn_sync};

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_warn_sync() {
        crate::context::Context::reset("test_warn_sync".to_string());
        info_sync!("test_warn_sync");
        warn_sync!("test_warn_sync Hello {world}!",world=23);
    }




    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn perfwarn() {
        use logwise::perfwarn;
        Context::reset("test_perfwarn".to_string());
        info_sync!("test_perfwarn");
        let _: i32 = perfwarn!("test_perfwarn interval name", {
         //code to profile
            23
        });

    }
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_debuginternal_sync() {
        crate::context::Context::reset("test_debuginternal_sync".to_string());
        debuginternal_sync!("test_debuginternal_sync");
    }
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_log_rich() {
        let val = false;
        crate::context::Context::reset("test_log_rich".to_string());

        debuginternal_sync!("Hello {world}!",world=val);

    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_log_custom() {
        crate::context::Context::reset("test_log_custom".to_string());
        #[derive(Debug)]
        #[allow(dead_code)]
        struct S(i32);
        let s = S(23);
        debuginternal_sync!("{s}!",s=logwise::privacy::LogIt(&s));
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_log_info_async() {
        crate::context::Context::reset("test_log_info_async".to_string());
        let _ = async {
            logwise::info_async!("test_log_info_async");
        };
        //I guess we do something with this, but we can't call truntime because it depends on us...
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_trace() {
        crate::context::Context::reset("test_trace".to_string());
        logwise::trace_sync!("test_trace");
    }
}