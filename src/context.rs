//SPDX-License-Identifier: MIT OR Apache-2.0
use std::cell::{Cell};
use std::collections::HashMap;
use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::task::Poll;
use logwise_proc::{debuginternal_sync};
use crate::Level;

static TASK_ID: AtomicU64 = AtomicU64::new(0);

static CONTEXT_ID: AtomicU64 = AtomicU64::new(0);



#[derive(Copy,Clone,Debug,PartialEq,Eq,Hash)]
pub struct TaskID(u64);
impl Display for TaskID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Copy,Clone,Debug,PartialEq,Eq,Hash)]
pub struct ContextID(u64);



impl Task {
    #[inline]
    fn add_task_interval(&self, key: &'static str, duration: std::time::Duration) {
        let mut borrow = self.mutable.lock().unwrap();
        borrow.interval_statistics.get_mut(key).map(|v| *v += duration).unwrap_or_else(|| {
            borrow.interval_statistics.insert(key, duration);
        });
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        use crate::logger::Logger;
        if !self.mutable.lock().unwrap().interval_statistics.is_empty() {
            let mut record = crate::log_record::LogRecord::new(Level::PerfWarn);
            //log task ID
            record.log_owned(format!("{} ",self.task_id.0));
            record.log("PERFWARN: statistics[");
            for (key, duration) in &self.mutable.lock().unwrap().interval_statistics {
                record.log(key);
                record.log_owned(format!(": {:?},", duration));
            }
            record.log("]");
            crate::global_logger::GLOBAL_LOGGER.finish_log_record(record);
        }

        if self.label != "Default task" {
            let mut record = crate::log_record::LogRecord::new(Level::Info);
            record.log_owned(format!("{} ", self.task_id.0));
            record.log("Finished task `");
            record.log(self.label);
            record.log("`");
            crate::global_logger::GLOBAL_LOGGER.finish_log_record(record);
        }
    }
}
#[derive(Clone,Debug)]
struct TaskMutable {
    interval_statistics: HashMap<&'static str, std::time::Duration>,
}
#[derive(Debug)]
pub struct Task {
    task_id: TaskID,
    mutable: Mutex<TaskMutable>,
    label: &'static str,
}

impl Task {
    fn new(label: &'static str) -> Task {
        Task {
            task_id: TaskID(TASK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
            mutable: Mutex::new(TaskMutable {
                interval_statistics: HashMap::new(),
            }),
            label,
        }
    }
}


/**
Provides a set of info that can be used by multiple logs.
*/
#[derive(Debug)]
pub struct Context {
    parent: Option<Arc<Context>>,
    context_id: u64,
    //if some, we define a new task ID for this context.
    define_task: Option<Task>,
    ///whether this context is currently tracing
    is_tracing: AtomicBool,
}

thread_local! {
    static CONTEXT: Cell<Arc<Context>> = Cell::new(Context::new_task(None,"Default task"));
}

impl Context {

    /**
    Returns the current context.
    */
    #[inline]
    pub fn current() -> Arc<Context> {
        CONTEXT.with(|c|
            //safety: we don't let anyone get a mutable reference to this
            unsafe{&*c.as_ptr()}.clone()
        )
    }



    pub fn task(&self) -> &Task {
        if let Some(task) = &self.define_task {
            task
        } else {
            self.parent.as_ref().expect("No parent context").task()
        }
    }


    /**
    Creates a new context.

    # Parameters
    * `parent` - the parent context.  If None, this context will be orphaned.
    * `label` - a debug label for the context.
    */
    #[inline]
    pub fn new_task(parent: Option<Arc<Context>>, label: &'static str) -> Arc<Context> {
        Arc::new(Context {
            parent,
            context_id: CONTEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            define_task: Some(Task::new(label)),
            is_tracing: AtomicBool::new(false),
        })
    }


    /**
    Sets a blank context
    */
    #[inline]
    pub fn reset(label: &'static str) {
        let new_context = Context::new_task(None,label);
        new_context.set_current();
    }

    /**
    Creates a new context with the current context as the parent.

    This preserves the parent's task.
    */
    pub fn from_parent(context: Arc<Context>) -> Arc<Context> {
        let is_tracing = context.is_tracing.load(Ordering::Relaxed);
        Arc::new(Context {
            parent: Some(context),
            context_id: CONTEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            define_task: None,
            is_tracing: AtomicBool::new(is_tracing),
        })
    }

    #[inline]
    pub fn task_id(&self) -> TaskID {
        self.task().task_id
    }

    /**
    Determines whether this context is currently tracing.
    */
    #[inline]
    pub fn is_tracing(&self) -> bool {
        self.is_tracing.load(Ordering::Relaxed)
    }

    /**
    Returns true if we are currently tracing.
*/
    #[inline]
    pub fn currently_tracing() -> bool {
        CONTEXT.with(|c| {
            //safety: we don't let anyone get a mutable reference to this
            unsafe{&*c.as_ptr()}.is_tracing.load(Ordering::Relaxed)
        })
    }

    /**
    Begins tracing for the current context.

    */
    pub fn begin_trace() {
        Context::current().is_tracing.store(true, Ordering::Relaxed);
        logwise::trace_sync!("Begin trace");
    }

    /**
    Sets the current context to this one.
    */
    pub fn set_current(self: Arc<Self>) {
        let new_label = self.task().label;
        //find the parent task
        let new_task_id = self.task_id();
        let old = CONTEXT.replace(self);
        if old.task_id() != new_task_id {
            //find parent task if applicable
            let current = Context::current();

            let mut search = &current;
            let parent_task;
            loop {
                if search.task_id() != new_task_id {
                    parent_task = Some(search.task_id());
                    break;
                }
                else {
                    match &search.parent {
                        Some(parent) => search = parent,
                        None => {
                            parent_task = None;
                            break;
                        },
                    }
                }
            }
            debuginternal_sync!("Enter task `{label}` parent: `{parent}`",label=new_label,parent=parent_task.map(|e| e.0.to_string()).unwrap_or("?".to_string()));
        }
    }

    /**
    Finds the number of nesting contexts.
    */
    pub fn nesting_level(&self) -> usize {
        let mut level = 0;
        let mut current = self;
        while let Some(parent) = &current.parent {
            level += 1;
            current = parent;
        }
        level
    }

    /**
    Returns the ID of the current context.
    */
    #[inline]
    pub fn context_id(&self) -> ContextID {
        ContextID(self.context_id)
    }





    /**
    Pops the context with specified ID.

    If the context cannot be found, panics.
    */
    pub fn pop(id: ContextID)  {
        let mut current = Context::current();
        loop {
            if current.context_id() == id {
                let parent = current.parent.clone().expect("No parent context");
                CONTEXT.replace(parent);
                return;
            }
            let parent = current.parent.as_ref().expect("No parent context").clone();
            current = parent;
        }
    }







    /**
    Internal implementation detail of the logging system.

    Logs the start of logs that typically use Context.
    */
    #[inline] pub fn _log_prelude(&self, record: &mut crate::log_record::LogRecord) {
        let prefix = if self.is_tracing() {
            "T"
        } else {
            " "
        };
        record.log(prefix);
        for _ in 0..self.nesting_level() {
            record.log(" ");
        }
        record.log_owned(format!("{} ",self.task_id()));
    }

    #[inline] pub fn _add_task_interval(&self, key: &'static str, duration: std::time::Duration) {
        self.task().add_task_interval(key, duration);
    }
}



/**
Applies the context to the given future for the duration of a poll event.

This can be used to inject a future with "plausible context" to hostile executors.
*/
pub struct ApplyContext<F>(Arc<Context>,F);

impl<F> ApplyContext<F> {
    /**
    Creates a new type given the context to apply and the future to poll
*/
    pub fn new(context: Arc<Context>, f: F) -> Self {
        Self(context, f)
    }
}

impl<F> Future for ApplyContext<F> where F: Future {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let (context, fut) = unsafe {
            let d = self.get_unchecked_mut();
            (d.0.clone(), Pin::new_unchecked(&mut d.1))
        };
        let id = context.context_id();
        context.set_current();
        let r = fut.poll(cx);
        Context::pop(id);
        r
    }
}


#[cfg(test)] mod tests {
    use super::Context;
    #[cfg(target_arch="wasm32")]
    use wasm_bindgen_test::*;
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_new_context() {
        Context::reset("test_new_context");
        let port_context = Context::current();
        let next_context = Context::from_parent(port_context);
        let next_context_id = next_context.context_id();
        next_context.set_current();

        Context::pop(next_context_id);
    }
}