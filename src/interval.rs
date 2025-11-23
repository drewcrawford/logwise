// SPDX-License-Identifier: MIT OR Apache-2.0
/*!
Defines our Interval types.

These represent 2 paired log values, such as a start and end time.
*/

use crate::Level;
use crate::context::Context;
use crate::global_logger::global_loggers;
use crate::log_record::LogRecord;

/// A performance warning interval that tracks execution time.
///
/// `PerfwarnInterval` measures the duration between its creation and destruction (drop),
/// logging the elapsed time as a performance warning. The duration is also accumulated
/// into the current task's statistics for aggregated reporting.
///
/// # Usage
///
/// This type is typically not created directly. Instead, use the `perfwarn!` or
/// `perfwarn_begin!` macros which properly set up the interval with source location
/// information.
///
/// # Example
///
/// ```rust
/// # fn perform_expensive_operation() {}
/// // Using the perfwarn! macro (recommended)
/// logwise::perfwarn!("database_query", {
///     perform_expensive_operation();
/// });
///
/// // Using perfwarn_begin! for more control
/// let interval = logwise::perfwarn_begin!("network_request");
/// perform_expensive_operation();
/// // Duration is logged when interval drops
/// drop(interval);
/// ```
///
/// # Scaling
///
/// Use the [`scale`](PerfwarnInterval::scale) method to adjust the reported duration when
/// only a portion of the measured time is relevant:
///
/// ```rust
/// # fn perform_expensive_operation() {}
/// let mut interval = logwise::perfwarn_begin!("partial_operation");
/// perform_expensive_operation();
/// // Report only 20% of the actual duration
/// interval.scale(0.2);
/// ```
#[derive(Debug)]
pub struct PerfwarnInterval {
    label: &'static str,
    start: crate::sys::Instant,
    scale: f32,
}

impl PerfwarnInterval {
    /**
        Creates a new interval.

        Do not use this manually, instead use the `perfwarn!` macro, or if you need to access the interval directly, use `perfwarn_begin!`.
    */
    #[inline]
    pub fn new(label: &'static str, time: crate::sys::Instant) -> Self {
        Self {
            label,
            start: time,
            scale: 1.0,
        }
    }

    #[inline]
    #[doc(hidden)]
    ///internal implementation detail
    pub fn log_timestamp(&self, record: &mut LogRecord) {
        let time = crate::sys::Instant::now();
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
        let end_time = crate::sys::Instant::now();
        let duration = end_time.duration_since(self.start);
        let ctx = Context::current();
        ctx._add_task_interval(self.label, duration);

        let mut record = LogRecord::new(Level::PerfWarn);
        let ctx = Context::current();
        ctx._log_prelude(&mut record);
        record.log("PERWARN: END ");
        record.log_time_since(end_time);

        record.log(self.label);
        record.log(" ");
        let scaled_duration = duration.mul_f32(self.scale);
        record.log_owned(format!("[interval took {:?}] ", scaled_duration));
        let global_loggers = global_loggers();
        for logger in global_loggers {
            logger.finish_log_record(record.clone());
        }
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

#[cfg(test)]
mod tests {
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn assert_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<super::PerfwarnInterval>();
        assert_send_sync::<super::PerfwarnIntervalIf>();
    }
}

/// A conditional performance warning interval that logs only if execution exceeds a threshold.
///
/// Unlike [`PerfwarnInterval`] which always logs, `PerfwarnIntervalIf` only emits a warning
/// if the measured duration exceeds the specified threshold. This is useful for operations
/// that are normally fast but occasionally slow down due to external factors.
///
/// # Usage
///
/// This type is typically not created directly. Instead, use the `perfwarn_begin_if!` macro
/// which properly sets up the interval with source location information.
///
/// # Example
///
/// ```rust
/// # use std::time::Duration;
/// # fn perform_database_query() {}
/// // Create a conditional interval that warns only if execution takes > 100ms
/// let _interval = logwise::perfwarn_begin_if!("database_query", Duration::from_millis(100));
/// perform_database_query();
/// // If the query took longer than 100ms, a warning is logged when _interval drops
/// ```
///
/// # Behavior
///
/// - The interval starts when created
/// - On drop, if the elapsed time exceeds the threshold:
///   - The duration is logged at the `PerfWarn` level
///   - The interval is added to the current task's statistics
/// - If the elapsed time is below the threshold, no log is emitted
#[derive(Debug)]
pub struct PerfwarnIntervalIf {
    label: &'static str,
    start: crate::sys::Instant,
    threshold: crate::sys::Duration,
    record: crate::log_record::LogRecord,
}

impl PerfwarnIntervalIf {
    #[inline]
    pub fn new(
        label: &'static str,
        time: crate::sys::Instant,
        threshold: crate::sys::Duration,
        record: crate::log_record::LogRecord,
    ) -> Self {
        Self {
            label,
            start: time,
            threshold,
            record,
        }
    }
}

impl Drop for PerfwarnIntervalIf {
    fn drop(&mut self) {
        let end_time = crate::sys::Instant::now();
        let duration = end_time.duration_since(self.start);
        let ctx = Context::current();
        ctx._add_task_interval_if(self.label, duration, self.threshold);

        if duration > self.threshold {
            // We need to insert the timestamp before the label (which is already in the record).
            // The record parts structure is: [Prelude] "PERFWARN: " File ":line:col " [Label]
            // We want: ... ":line:col " [Timestamp] [Label] " is SLOW: " [Duration]

            let mut insert_idx = 0;
            let mut found = false;
            for (i, part) in self.record.parts.iter().enumerate() {
                if part == "PERFWARN: " {
                    // Found the marker. The next two parts are File and LineCol.
                    // So we want to insert at i + 3.
                    insert_idx = i + 3;
                    found = true;
                    break;
                }
            }

            if found && insert_idx <= self.record.parts.len() {
                // Generate timestamp string
                let mut temp_record = crate::log_record::LogRecord::new(Level::PerfWarn);
                temp_record.log_time_since(end_time);
                let timestamp = temp_record.parts.pop().unwrap();

                self.record.parts.insert(insert_idx, timestamp);
            }

            self.record.log(" is SLOW: ");
            self.record
                .log_owned(format!("[interval took {:?}] ", duration));

            let global_loggers = global_loggers();
            for logger in global_loggers {
                logger.finish_log_record(self.record.clone());
            }
        }
    }
}
