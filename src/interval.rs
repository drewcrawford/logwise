/*!
Defines our Interval types.

These represent 2 paired log values, such as a start and end time.
*/

use crate::context::{Context, ContextID};
use crate::global_logger::GLOBAL_LOGGER;
use crate::log_record::LogRecord;
use crate::logger::Logger;

#[derive(Debug)]
pub struct PerfwarnInterval {
    label: &'static str,
    start: std::time::Instant,
    context_id: ContextID,
    scale: f32,
}

impl PerfwarnInterval {
    /**
    Creates a new interval.

    Do not use this manually, instead use the `perfwarn!` macro, or if you need to access the interval directly, use `perfwarn_begin!`.
*/
    #[inline]
    pub fn new(label: &'static str, time: std::time::Instant) -> Self {
        let context_id = Context::new_push();
        Self {
            label,
            start: time,
            context_id,
            scale: 1.0,
        }
    }

    #[inline]
    #[doc(hidden)]
    ///internal implementation detail
    pub fn log_timestamp(&self, record: &mut LogRecord) {
        let time = std::time::Instant::now();
        let duration = time.duration_since(self.start);
        record.log_owned(format!("[{:?}] ", duration));
    }

    /**
    Cause the reported time interval to be scaled by the amount.

    Consider a case where we're warning about "some subset of the interval".  For example,
    let's say by tweaking constants we can get a 20% speedup.  Warning about the entire interval
    would be misleading.  Instead, we can scale the interval by 0.2 to reflect the subset.
*/
    pub fn scale(&mut self, scale: f32) {
        self.scale = scale;
    }

}

impl Drop for PerfwarnInterval {
    fn drop(&mut self) {
        let end_time = std::time::Instant::now();
        let duration = end_time.duration_since(self.start);
        let ctx = Context::current();
        ctx._add_task_interval(self.label, duration);

        Context::pop(self.context_id);
        let mut record = LogRecord::new();
        unsafe {
            let ctx = (&*Context::current());
            ctx._log_prelude(&mut record);
        }
        record.log("PERWARN: END ");
        record.log_time_since(end_time);


        record.log(self.label);
        record.log(" ");
        let scaled_duration = duration.mul_f32(self.scale);
        record.log_owned(format!("[interval took {:?}] ", scaled_duration));
        GLOBAL_LOGGER.finish_log_record(record);

    }
}

/*
boilerplate notes.

1.  Copy, clone, no.  We don't want to copy the context?
2.  PartialEq, Ord, etc.  No, we don't want to compare these.
3.  Default, no, we don't want to create these without a start time.
4.  display, not really.
5.  From/Into, I think we want to avoid creating this without a line number.
6.  AsRef/AsMut, there's really nothing to desugar to
7.  Deref, similar
8.  Send/Sync, Probably?
 */

#[cfg(test)] mod tests {
    #[test] fn assert_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<super::PerfwarnInterval>();
    }
}