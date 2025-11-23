// SPDX-License-Identifier: MIT OR Apache-2.0

//! Task management and performance tracking.

use crate::Level;
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;

pub(crate) static TASK_ID: AtomicU64 = AtomicU64::new(0);

/// Unique identifier for a task.
///
/// Each task gets a monotonically increasing ID that is unique across the entire
/// process lifetime. This ID is used in log output to correlate related log messages.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskID(pub(crate) u64);

impl Display for TaskID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TaskMutable {
    pub(crate) interval_statistics: HashMap<&'static str, crate::sys::Duration>,
    pub(crate) interval_thresholds: HashMap<&'static str, crate::sys::Duration>,
}

/// Represents a logical unit of work with performance tracking.
///
/// Tasks automatically log their lifecycle (creation and completion) and collect
/// performance statistics from perfwarn intervals. When a task is dropped, it logs
/// any accumulated performance statistics.
///
/// Tasks are typically created indirectly through [`Context::new_task`](super::Context::new_task):
///
/// ```rust
/// use logwise::context::Context;
///
/// let ctx = Context::new_task(None, "data_processing".to_string(), logwise::Level::Info, true);
/// ctx.set_current();
/// // Task lifecycle is automatically logged
/// ```
#[derive(Debug)]
pub struct Task {
    pub(crate) task_id: TaskID,
    pub(crate) mutable: Mutex<TaskMutable>,
    pub(crate) label: String,
    pub(crate) completion_level: Level,
    pub(crate) should_log_completion: bool,
}

impl Task {
    /// Creates a new task with the given label.
    ///
    /// This is an internal method; tasks are typically created through [`Context::new_task`](super::Context::new_task).
    pub(crate) fn new(label: String, completion_level: Level, should_log_completion: bool) -> Task {
        Task {
            task_id: TaskID(TASK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
            mutable: Mutex::new(TaskMutable {
                interval_statistics: HashMap::new(),
                interval_thresholds: HashMap::new(),
            }),
            label,
            completion_level,
            should_log_completion,
        }
    }

    #[inline]
    pub(crate) fn add_task_interval(&self, key: &'static str, duration: crate::sys::Duration) {
        let mut borrow = self.mutable.lock().unwrap();
        borrow
            .interval_statistics
            .get_mut(key)
            .map(|v| *v += duration)
            .unwrap_or_else(|| {
                borrow.interval_statistics.insert(key, duration);
            });
    }

    #[inline]
    pub(crate) fn add_task_interval_if(
        &self,
        key: &'static str,
        duration: crate::sys::Duration,
        threshold: crate::sys::Duration,
    ) {
        let mut borrow = self.mutable.lock().unwrap();
        borrow
            .interval_statistics
            .get_mut(key)
            .map(|v| *v += duration)
            .unwrap_or_else(|| {
                borrow.interval_statistics.insert(key, duration);
            });

        borrow
            .interval_thresholds
            .get_mut(key)
            .map(|v| *v += threshold)
            .unwrap_or_else(|| {
                borrow.interval_thresholds.insert(key, threshold);
            });
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        let borrow = self.mutable.lock().unwrap();
        if !borrow.interval_statistics.is_empty() {
            let mut record = crate::log_record::LogRecord::new(Level::PerfWarn);
            //log task ID
            record.log_owned(format!("{} ", self.task_id.0));
            record.log("PERFWARN: statistics[");
            let mut first = true;
            for (key, duration) in &borrow.interval_statistics {
                // Check if we should log this statistic
                if let Some(threshold) = borrow.interval_thresholds.get(key) {
                    if duration <= threshold {
                        continue;
                    }
                }

                if !first {
                    record.log(", ");
                }
                first = false;
                record.log(key);
                record.log_owned(format!(": {:?}", duration));
            }
            record.log("]");

            // Only log if we actually added any statistics
            if !first {
                let global_loggers = crate::global_logger::global_loggers();
                for logger in global_loggers {
                    logger.finish_log_record(record.clone());
                }
            }
        }

        if self.should_log_completion {
            let mut record = crate::log_record::LogRecord::new(self.completion_level);
            record.log_owned(format!("{} ", self.task_id.0));
            record.log("Finished task `");
            record.log(&self.label);
            record.log("`");
            let global_loggers = crate::global_logger::global_loggers();
            for logger in global_loggers {
                logger.finish_log_record(record.clone());
            }
        }
    }
}
