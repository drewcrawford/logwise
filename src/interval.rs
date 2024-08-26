/*!
Defines our Interval types.

These represent 2 paired log values, such as a start and end time.
*/

use crate::context::{Context, ContextID};
use crate::global_logger::GLOBAL_LOGGER;
use crate::log_record::LogRecord;
use crate::logger::Logger;

pub struct PerfwarnInterval {
    label: &'static str,
    start: std::time::Instant,
    context_id: ContextID,
}

impl PerfwarnInterval {
    #[inline]
    pub fn new(label: &'static str, time: std::time::Instant) -> Self {
        let context_id = Context::new_push();
        Self {
            label,
            start: time,
            context_id,
        }
    }

    #[inline]
    pub fn log_timestamp(&self, record: &mut LogRecord) {
        let time = std::time::Instant::now();
        let duration = time.duration_since(self.start);
        record.log_owned(format!("[{:?}] ", duration));
    }


}

impl Drop for PerfwarnInterval {
    fn drop(&mut self) {
        let end_time = std::time::Instant::now();

        Context::pop(self.context_id);
        let mut record = LogRecord::new();
        unsafe {
            let ctx = (&*Context::current()).as_ref().unwrap();
            ctx._log_prelude(&mut record);
        }
        record.log("PERWARN: END ");
        record.log_time_since(end_time);


        record.log(self.label);
        record.log(" ");
        let duration = end_time.duration_since(self.start);
        record.log_owned(format!("[interval took {:?}] ", duration));
        GLOBAL_LOGGER.finish_log_record(record);

    }
}
