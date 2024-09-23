use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt::Display;
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
#[derive(Clone)]
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
#[derive(Clone)]
pub struct Context {
    parent: Option<Box<Context>>,
    context_id: u64,
    _mutable_context: RefCell<MutableContext>,
    //if some, we define a new task ID for this context.
    define_task: Option<Task>
}

thread_local! {
    static CONTEXT: Cell<Option<Context>> = const{Cell::new(None)};
}

impl Context {
    /**
    Determines whether a context is currently set.
    */
    #[inline]
    pub fn has_current() -> bool {
        CONTEXT.with(|c| {
            let context = c.take();
            let result = context.is_some();
            c.set(context);
            result
        })
    }
    /**
    Returns a raw pointer to the thread-local context.

    When using this pointer, you must ensure that the context is only accessed from the local thread,
    and is not mutated by the local thread while the pointer is in use.

    If you can stand a safe API, see [current_clone] as an alternative.
    */
    #[inline]
    pub fn current() -> *const Option<Context> {
        CONTEXT.with(|c| {
            c.as_ptr()
        })
    }

    /**
    Clones the current context into the value.

    This may be used to get a copy of the context for use in a closure.
    */
    #[inline]
    pub fn current_clone() -> Option<Context> {
        unsafe{&*Self::current()}.clone()
    }


    pub fn task(&self) -> Option<&Task> {
        if let Some(task) = &self.define_task {
            Some(task)
        } else {
            self.parent.as_ref().and_then(|p| p.task())
        }
    }


    /**
    Creates a new orphaned context
    */
    #[inline]
    pub fn new_task() -> Context {
        Context {
            parent: None,
            _mutable_context: RefCell::new(MutableContext {

            }),
            context_id: CONTEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            define_task: Some(Task::new()),
        }
    }

    /**
    Sets a blank context
    */
    #[inline]
    pub fn reset() {
        CONTEXT.with(|c| c.set(Some(Context::new_task())));
    }

    /**
    Creates a new context with the current context as the parent.
    */
    pub fn from_context(context: Context) -> Context {
        Context {
            parent: Some(Box::new(context)),
            _mutable_context: RefCell::new(MutableContext {

            }),
            context_id: CONTEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            define_task: None,
        }
    }

    #[inline]
    pub fn task_id(&self) -> TaskID {
        self.task().unwrap().task_id
    }

    /**
    Sets the current context to this one.
    */
    pub fn set_current(self) {
        CONTEXT.set(Some(self));
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
    Pushes the current context onto the stack and sets this context as the current one.
    */
    pub(crate) fn new_push() -> ContextID {
        let current = CONTEXT.with(|c| c.take()).unwrap();
        let new_context = Context {
            parent: Some(Box::new(current)),
            context_id: CONTEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            _mutable_context: RefCell::new(MutableContext {  }),
            define_task: None,
        };
        let id = new_context.context_id();
        CONTEXT.with(|c| c.set(Some(new_context)));
        id
    }



    /**
    Pops the context with specified ID.

    If the context cannot be found, panics.
    */
    pub(crate)fn pop(id: ContextID) {
        let mut current = CONTEXT.with(|c| c.take()).unwrap();
        loop {
            if current.context_id() == id {
                let parent = current.parent.take().unwrap();
                CONTEXT.set(Some(*parent));
                return;
            }
            let parent = current.parent.take().unwrap();
            current = *parent;
        }
    }






    /**
    Internal implementation detail of the logging system.

    # Safety
    You must guarantee that the context is not destroyed or mutated while the reference is in use.
    */
    #[inline]
    pub unsafe fn _log_current_context(file: &'static str,line: u32, column: u32) -> &'static Context {
        use crate::logger::Logger;
        match (&*Context::current()).as_ref() {
            None => {
                /*
                Create an additional warning about the missing context.
                 */
                let mut record = crate::log_record::LogRecord::new();
                record.log("WARN: ");

                //file, line
                record.log(file);
                record.log_owned(format!(":{}:{} ",line,column));

                //for warn, we can afford timestamp
                record.log_timestamp();
                record.log("No context found. Creating new task context.");

                let global_logger = &crate::global_logger::GLOBAL_LOGGER;
                global_logger.finish_log_record(record);
                let new_ctx = Context::new_task();
                new_ctx.set_current();
                (&*Context::current()).as_ref().unwrap()
            }
            Some(ctx) => {
                ctx
            }

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
        let port_context = Context::current_clone().unwrap();
        let next_context = Context::from_context(port_context);
        next_context.clone().set_current();

        Context::pop(next_context.context_id());
    }
}