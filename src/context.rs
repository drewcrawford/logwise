//SPDX-License-Identifier: MIT OR Apache-2.0
use std::cell::{Cell};
use std::collections::HashMap;
use std::fmt::Display;
use std::future::Future;
use std::hash::{Hash, Hasher};
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
    fn add_task_interval(&self, key: &'static str, duration: crate::sys::Duration) {
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
    interval_statistics: HashMap<&'static str, crate::sys::Duration>,
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
Internal context data.
*/
#[derive(Debug)]
struct ContextInner {
    parent: Option<Context>,
    context_id: u64,
    //if some, we define a new task ID for this context.
    define_task: Option<Task>,
    ///whether this context is currently tracing
    is_tracing: AtomicBool,
}

/**
Provides a set of info that can be used by multiple logs.
*/
#[derive(Debug, Clone)]
pub struct Context {
    inner: Arc<ContextInner>,
}

impl PartialEq for Context {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for Context {}

impl Hash for Context {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.inner).hash(state);
    }
}

impl Display for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let nesting = self.nesting_level();
        write!(f, "{}{} ({})", 
               "  ".repeat(nesting),
               self.task_id(), 
               self.task().label)
    }
}

impl AsRef<Task> for Context {
    fn as_ref(&self) -> &Task {
        self.task()
    }
}


thread_local! {
    static CONTEXT: Cell<Context> = Cell::new(Context::new_task(None,"Default task"));
}

impl Context {

    /**
    Returns the current context.
    */
    #[inline]
    pub fn current() -> Context {
        CONTEXT.with(|c|
            //safety: we don't let anyone get a mutable reference to this
            unsafe{&*c.as_ptr()}.clone()
        )
    }



    pub fn task(&self) -> &Task {
        if let Some(task) = &self.inner.define_task {
            task
        } else {
            self.inner.parent.as_ref().expect("No parent context").task()
        }
    }


    /**
    Creates a new context.

    # Parameters
    * `parent` - the parent context.  If None, this context will be orphaned.
    * `label` - a debug label for the context.
    */
    #[inline]
    pub fn new_task(parent: Option<Context>, label: &'static str) -> Context {
        Context {
            inner: Arc::new(ContextInner {
                parent,
                context_id: CONTEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                define_task: Some(Task::new(label)),
                is_tracing: AtomicBool::new(false),
            })
        }
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
    pub fn from_parent(context: Context) -> Context {
        let is_tracing = context.inner.is_tracing.load(Ordering::Relaxed);
        Context {
            inner: Arc::new(ContextInner {
                parent: Some(context),
                context_id: CONTEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                define_task: None,
                is_tracing: AtomicBool::new(is_tracing),
            })
        }
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
        self.inner.is_tracing.load(Ordering::Relaxed)
    }

    /**
    Returns true if we are currently tracing.
*/
    #[inline]
    pub fn currently_tracing() -> bool {
        CONTEXT.with(|c| {
            //safety: we don't let anyone get a mutable reference to this
            unsafe{&*c.as_ptr()}.inner.is_tracing.load(Ordering::Relaxed)
        })
    }

    /**
    Begins tracing for the current context.

    */
    pub fn begin_trace() {
        Context::current().inner.is_tracing.store(true, Ordering::Relaxed);
        logwise::trace_sync!("Begin trace");
    }

    /**
    Sets the current context to this one.
    */
    pub fn set_current(self) {
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
                    match &search.inner.parent {
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
        while let Some(parent) = &current.inner.parent {
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
        ContextID(self.inner.context_id)
    }





    /**
    Pops the context with specified ID.

    If the context cannot be found, logs a warning.
    */
    pub fn pop(id: ContextID)  {
        let mut current = Context::current();
        loop {
            if current.context_id() == id {
                let parent = current.inner.parent.clone().expect("No parent context");
                CONTEXT.replace(parent);
                return;
            }
            match current.inner.parent.as_ref() {
                None => {
                    logwise::warn_sync!("Tried to pop context with ID {id}, but it was not found in the current context chain.", id=id.0);
                    return;
                },
                Some(ctx) => {
                    current = ctx.clone()
                }
            }
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

    #[inline] pub fn _add_task_interval(&self, key: &'static str, duration: crate::sys::Duration) {
        self.task().add_task_interval(key, duration);
    }
}



/**
Applies the context to the given future for the duration of a poll event.

This can be used to inject a future with "plausible context" to hostile executors.
*/
pub struct ApplyContext<F>(Context,F);

impl<F> ApplyContext<F> {
    /**
    Creates a new type given the context to apply and the future to poll
*/
    pub fn new(context: Context, f: F) -> Self {
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
    use super::{Context, Task, TaskID};
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

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_context_equality() {
        Context::reset("test_context_equality");
        let context1 = Context::current();
        let context2 = context1.clone();
        let context3 = Context::new_task(None, "different_task");

        // Same Arc pointer should be equal
        assert_eq!(context1, context2);
        
        // Different Arc pointers should not be equal
        assert_ne!(context1, context3);
        assert_ne!(context2, context3);
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_context_hash() {
        use std::collections::HashMap;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        Context::reset("test_context_hash");
        let context1 = Context::current();
        let context2 = context1.clone();
        let context3 = Context::new_task(None, "different_task");

        // Same Arc pointer should have same hash
        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        context1.hash(&mut hasher1);
        context2.hash(&mut hasher2);
        assert_eq!(hasher1.finish(), hasher2.finish());

        // Different Arc pointers should have different hashes (highly likely)
        let mut hasher3 = DefaultHasher::new();
        context3.hash(&mut hasher3);
        assert_ne!(hasher1.finish(), hasher3.finish());

        // Test that Context can be used as HashMap key
        let mut map = HashMap::new();
        map.insert(context1.clone(), "value1");
        map.insert(context3.clone(), "value3");

        assert_eq!(map.get(&context1), Some(&"value1"));
        assert_eq!(map.get(&context2), Some(&"value1")); // same as context1
        assert_eq!(map.get(&context3), Some(&"value3"));
        assert_eq!(map.len(), 2); // only 2 unique contexts
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_context_display() {
        Context::reset("root_task");
        let root_context = Context::current();
        
        // Root context should have no indentation (nesting level 0)
        let root_display = format!("{}", root_context);
        assert!(root_display.starts_with(&format!("{} (root_task)", root_context.task_id())));
        assert!(!root_display.starts_with("  ")); // no indentation
        
        // Create a child context
        let child_context = Context::from_parent(root_context.clone());
        child_context.clone().set_current();
        let child_display = format!("{}", child_context);
        
        // Child should have 1 level of indentation
        assert!(child_display.starts_with("  ")); // 2 spaces for nesting level 1
        assert!(child_display.contains(&format!("{} (root_task)", root_context.task_id())));
        
        // Create a new task context as child
        let task_context = Context::new_task(Some(child_context.clone()), "child_task");
        task_context.clone().set_current();
        let task_display = format!("{}", task_context);
        
        // Task context should have 2 levels of indentation
        assert!(task_display.starts_with("    ")); // 4 spaces for nesting level 2
        assert!(task_display.contains(&format!("{} (child_task)", task_context.task_id())));
        
        // Create grandchild
        let grandchild_context = Context::from_parent(task_context.clone());
        grandchild_context.clone().set_current();
        let grandchild_display = format!("{}", grandchild_context);
        
        // Grandchild should have 3 levels of indentation
        assert!(grandchild_display.starts_with("      ")); // 6 spaces for nesting level 3
        assert!(grandchild_display.contains(&format!("{} (child_task)", task_context.task_id())));
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_context_as_ref_task() {
        Context::reset("test_as_ref");
        let context = Context::current();
        
        // Test that AsRef<Task> works
        let task_ref: &Task = context.as_ref();
        assert_eq!(task_ref.task_id, context.task_id());
        assert_eq!(task_ref.label, "test_as_ref");
        
        // Test that we can use Context where &Task is expected
        fn takes_task_ref(task: &Task) -> TaskID {
            task.task_id
        }
        
        // Test explicit AsRef usage
        let id1 = takes_task_ref(context.as_ref());
        assert_eq!(id1, context.task_id());
        
        // Test with generic function that accepts AsRef<Task>
        fn takes_as_ref_task<T: AsRef<Task>>(item: T) -> TaskID {
            item.as_ref().task_id
        }
        
        let id2 = takes_as_ref_task(&context);
        let id3 = takes_as_ref_task(context.clone());
        assert_eq!(id1, id2);
        assert_eq!(id2, id3);
        
        // Test with different context types
        let child_context = Context::from_parent(context.clone());
        let child_task_ref: &Task = child_context.as_ref();
        
        // Child should have same task as parent (since from_parent preserves task)
        assert_eq!(child_task_ref.task_id, context.task_id());
        assert_eq!(child_task_ref.label, "test_as_ref");
        
        let new_task_context = Context::new_task(Some(context.clone()), "new_task");
        let new_task_ref: &Task = new_task_context.as_ref();
        
        // New task should have different ID and label
        assert_ne!(new_task_ref.task_id, context.task_id());
        assert_eq!(new_task_ref.label, "new_task");
    }
}