use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

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
impl Display for ContextID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}


impl Task {
    #[inline]
    fn add_task_interval(&self, key: &'static str, duration: std::time::Duration) {
        let mut borrow = self.mutable.borrow_mut();
        borrow.interval_statistics.get_mut(key).map(|v| *v += duration).unwrap_or_else(|| {
            borrow.interval_statistics.insert(key, duration);
        });
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        use crate::logger::Logger;
        if !self.mutable.borrow().interval_statistics.is_empty() {
            let mut record = crate::log_record::LogRecord::new();
            //log task ID
            record.log_owned(format!("{} ",self.task_id.0));
            record.log("PERFWARN: statistics[");
            for (key, duration) in &self.mutable.borrow().interval_statistics {
                record.log(key);
                record.log_owned(format!(": {:?},", duration));
            }
            record.log("]");
            crate::global_logger::GLOBAL_LOGGER.finish_log_record(record);
        }

        let mut record = crate::log_record::LogRecord::new();
        record.log_owned(format!("{} ",self.task_id.0));
        record.log("Finished task");
        crate::global_logger::GLOBAL_LOGGER.finish_log_record(record);
    }
}
#[derive(Clone)]
struct TaskMutable {
    interval_statistics: HashMap<&'static str, std::time::Duration>,
}
pub struct Task {
    task_id: TaskID,
    mutable: RefCell<TaskMutable>,
}

impl Task {
    fn new() -> Task {
        Task {
            task_id: TaskID(TASK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
            mutable: RefCell::new(TaskMutable {
                interval_statistics: HashMap::new(),
            }),
        }
    }
}
#[derive(Clone)]
struct MutableContext {
}


/**
Provides a set of info that can be used by multiple logs.
*/
pub struct Context {
    parent: Option<Arc<Context>>,
    context_id: u64,
    _mutable_context: RefCell<MutableContext>,
    //if some, we define a new task ID for this context.
    define_task: Option<Task>,
    ///whether this context is currently tracing
    is_tracing: bool,
}

thread_local! {
    static CONTEXT: Cell<Arc<Context>> = Cell::new(Arc::new(Context::new_task(None)));
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



    pub fn task(&self) -> Option<&Task> {
        if let Some(task) = &self.define_task {
            Some(task)
        } else {
            self.parent.as_ref().and_then(|p| p.task())
        }
    }


    /**
    Creates a new orphaned context.


    */
    #[inline]
    pub fn new_task(parent: Option<Arc<Context>>) -> Context {
        let is_tracing = parent.as_ref().map(|e|e.is_tracing).unwrap_or(false);

        Context {
            parent,
            _mutable_context: RefCell::new(MutableContext {

            }),
            context_id: CONTEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            define_task: Some(Task::new()),
            is_tracing
        }
    }


    /**
    Sets a blank context
    */
    #[inline]
    pub fn reset() {
        CONTEXT.with(|c| c.replace(Arc::new(Context::new_task(None))));
    }

    /**
    Creates a new context with the current context as the parent.
    */
    pub fn from_parent(context: Arc<Context>) -> Context {
        let is_tracing = context.is_tracing;
        Context {
            parent: Some(context),
            _mutable_context: RefCell::new(MutableContext {

            }),
            context_id: CONTEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            define_task: None,
            is_tracing,
        }
    }

    #[inline]
    pub fn task_id(&self) -> TaskID {
        self.task().unwrap().task_id
    }

    /**
    Determines whether this context is currently tracing.
    */
    #[inline]
    pub const fn is_tracing(&self) -> bool {
        self.is_tracing
    }

    /**
    Returns true if we are currently tracing.
*/
    #[inline]
    pub fn currently_tracing() -> bool {
        CONTEXT.with(|c| {
            //safety: we don't let anyone get a mutable reference to this
            unsafe{&*c.as_ptr()}.is_tracing
        })
    }

    /**
    Begins tracing for the current context.

    */
    pub fn begin_trace() {
        CONTEXT.with(|c| {
            //safety: we don't let anyone get a mutable reference to this
            todo!()
        });
    }

    /**
    Sets the current context to this one.
    */
    pub fn set_current(self) {
        CONTEXT.replace(Arc::new(self));
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
    pub fn pop(id: ContextID) {
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
        for _ in 0..self.nesting_level() {
            record.log(" ");
        }
        record.log_owned(format!("{} ",self.task_id()));
    }

    #[inline] pub fn _add_task_interval(&self, key: &'static str, duration: std::time::Duration) {
        self.task().as_ref()
            .expect("No current task").add_task_interval(key, duration);
    }
}

#[cfg(test)] mod tests {
    use super::Context;

    #[test] fn test_new_context() {
        Context::reset();
        let port_context = Context::current();
        let next_context = Context::from_parent(port_context);
        let next_context_id = next_context.context_id();
        next_context.set_current();

        Context::pop(next_context_id);
    }
}