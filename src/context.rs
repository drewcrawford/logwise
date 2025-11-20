// SPDX-License-Identifier: MIT OR Apache-2.0

//! Thread-local context management for hierarchical, task-based logging.
//!
//! This module provides the core context system that enables logwise to track tasks,
//! manage hierarchical logging contexts, and collect performance statistics. Contexts
//! are thread-local and form a parent-child hierarchy that allows for structured logging
//! with automatic indentation and task tracking.
//!
//! # Overview
//!
//! The context system consists of three main components:
//!
//! - [`Context`]: A hierarchical logging context that can represent either a new task or
//!   inherit from a parent context
//! - [`Task`]: Represents a logical unit of work with performance tracking capabilities
//! - [`ApplyContext`]: A [`Future`] wrapper that preserves context across executor boundaries
//!
//! # Thread-Local Context Management
//!
//! Each thread maintains its own current context via thread-local storage. The context
//! forms a linked list from child to parent, enabling hierarchical task tracking:
//!
//! ```rust
//! use logwise::context::Context;
//!
//! // Set a new root context for this thread
//! Context::reset("root_task".to_string());
//!
//! // Get the current context
//! let current = Context::current();
//!
//! // Create a child context that inherits from the current one
//! let child = Context::from_parent(current.clone());
//! child.set_current();
//! ```
//!
//! # Task-Based Logging
//!
//! Tasks represent logical units of work and automatically log their lifecycle:
//!
//! ```rust
//! use logwise::context::Context;
//!
//! // Create a new task context
//! let task_ctx = Context::new_task(
//!     Some(Context::current()),
//!     "data_processing".to_string(),
//!     logwise::Level::Info,
//! );
//! task_ctx.clone().set_current();
//!
//! // The task automatically logs when it's created and dropped
//! // Any performance statistics collected during the task are logged on drop
//! ```
//!
//! # Performance Tracking
//!
//! The context system integrates with logwise's performance tracking to collect
//! statistics about task execution:
//!
//! ```rust
//! # use logwise::context::Context;
//! # Context::reset("test".to_string());
//! // Performance intervals are automatically added to the current task
//! // When using the perfwarn! macro:
//! logwise::perfwarn!("expensive_operation", {
//!     // This duration is automatically tracked in the current context's task
//!     std::thread::sleep(std::time::Duration::from_millis(100));
//! });
//! // Statistics are logged when the task is dropped
//! ```
//!
//! # Async Context Preservation
//!
//! The [`ApplyContext`] wrapper ensures contexts are preserved across async boundaries,
//! particularly useful with executors that don't preserve thread-local state:
//!
//! ```rust
//! use logwise::context::{Context, ApplyContext};
//! # async fn async_operation() {}
//!
//! # async fn example() {
//! let ctx = Context::new_task(None, "async_task".to_string(), logwise::Level::Info);
//!
//! // Wrap the future to preserve context during polling
//! let future = ApplyContext::new(ctx, async_operation());
//! future.await;
//! # }
//! ```
//!
//! # Tracing Support
//!
//! Contexts support selective tracing for detailed debugging:
//!
//! ```rust
//! use logwise::context::Context;
//!
//! // Enable tracing for the current context and its children
//! Context::begin_trace();
//!
//! // Check if currently tracing
//! if Context::currently_tracing() {
//!     // Trace-level logs will be enabled
//! }
//! ```

use crate::Level;
use std::cell::Cell;
use std::collections::HashMap;
use std::fmt::Display;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Poll;

static TASK_ID: AtomicU64 = AtomicU64::new(0);

static CONTEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Unique identifier for a task.
///
/// Each task gets a monotonically increasing ID that is unique across the entire
/// process lifetime. This ID is used in log output to correlate related log messages.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskID(u64);

impl Display for TaskID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a context.
///
/// Each context gets a unique ID that can be used to identify and pop specific
/// contexts from the context stack.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContextID(u64);

impl Task {
    #[inline]
    fn add_task_interval(&self, key: &'static str, duration: crate::sys::Duration) {
        let mut borrow = self.mutable.lock().unwrap();
        borrow
            .interval_statistics
            .get_mut(key)
            .map(|v| *v += duration)
            .unwrap_or_else(|| {
                borrow.interval_statistics.insert(key, duration);
            });
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        if !self.mutable.lock().unwrap().interval_statistics.is_empty() {
            let mut record = crate::log_record::LogRecord::new(Level::PerfWarn);
            //log task ID
            record.log_owned(format!("{} ", self.task_id.0));
            record.log("PERFWARN: statistics[");
            for (key, duration) in &self.mutable.lock().unwrap().interval_statistics {
                record.log(key);
                record.log_owned(format!(": {:?},", duration));
            }
            record.log("]");
            let global_loggers = crate::global_logger::global_loggers();
            for logger in global_loggers {
                logger.finish_log_record(record.clone());
            }
        }

        if self.label != "Default task" && crate::log_enabled!(self.completion_level) {
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
#[derive(Clone, Debug)]
struct TaskMutable {
    interval_statistics: HashMap<&'static str, crate::sys::Duration>,
}

/// Represents a logical unit of work with performance tracking.
///
/// Tasks automatically log their lifecycle (creation and completion) and collect
/// performance statistics from perfwarn intervals. When a task is dropped, it logs
/// any accumulated performance statistics.
///
/// Tasks are typically created indirectly through [`Context::new_task`]:
///
/// ```rust
/// use logwise::context::Context;
///
/// let ctx = Context::new_task(None, "data_processing".to_string(), logwise::Level::Info);
/// ctx.set_current();
/// // Task lifecycle is automatically logged
/// ```
#[derive(Debug)]
pub struct Task {
    task_id: TaskID,
    mutable: Mutex<TaskMutable>,
    label: String,
    completion_level: Level,
}

impl Task {
    /// Creates a new task with the given label.
    ///
    /// This is an internal method; tasks are typically created through [`Context::new_task`].
    fn new(label: String, completion_level: Level) -> Task {
        Task {
            task_id: TaskID(TASK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
            mutable: Mutex::new(TaskMutable {
                interval_statistics: HashMap::new(),
            }),
            label,
            completion_level,
        }
    }
}

/// Internal context data.
///
/// This structure holds the actual context state, wrapped in an Arc for cheap cloning.
#[derive(Debug)]
struct ContextInner {
    parent: Option<Context>,
    context_id: u64,
    /// If Some, this context defines a new task. If None, inherits task from parent.
    define_task: Option<Task>,
    /// Whether this context is currently tracing.
    is_tracing: AtomicBool,
}

/// Hierarchical logging context for task-based structured logging.
///
/// A `Context` represents a point in a hierarchical tree of logging contexts.
/// Each context can either define a new task or inherit its parent's task.
/// Contexts are cheap to clone (Arc-based) and thread-safe.
///
/// # Context Hierarchy
///
/// Contexts form a parent-child hierarchy that enables:
/// - Automatic log indentation based on nesting level
/// - Task inheritance (child contexts can share parent's task)
/// - Tracing propagation (trace settings flow to children)
///
/// # Examples
///
/// ## Basic Context Creation
///
/// ```rust
/// use logwise::context::Context;
///
/// // Create a root context (no parent)
/// let root = Context::new_task(None, "root_operation".to_string(), logwise::Level::Info);
/// root.clone().set_current();
///
/// // Create a child context with a new task
/// let child = Context::new_task(
///     Some(Context::current()),
///     "child_operation".to_string(),
///     logwise::Level::Info,
/// );
/// child.clone().set_current();
///
/// // Create a context that inherits the parent's task
/// let sibling = Context::from_parent(Context::current());
/// sibling.set_current();
/// ```
///
/// ## Context Popping
///
/// ```rust
/// use logwise::context::Context;
///
/// Context::reset("root".to_string());
/// let ctx = Context::from_parent(Context::current());
/// let ctx_id = ctx.context_id();
/// ctx.set_current();
///
/// // Later, pop back to the parent
/// Context::pop(ctx_id);
/// ```
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
        write!(
            f,
            "{}{} ({})",
            "  ".repeat(nesting),
            self.task_id(),
            self.task().label
        )
    }
}

impl AsRef<Task> for Context {
    fn as_ref(&self) -> &Task {
        self.task()
    }
}

thread_local! {
    static CONTEXT: Cell<Context> = Cell::new(Context::new_task_internal(
        None,
        "Default task".to_string(),
        Level::DebugInternal,
        0,
    ));
}

impl Context {
    /// Returns the current context for this thread.
    ///
    /// This retrieves the thread-local context that will be used for logging.
    /// Every thread starts with a default context that can be replaced using
    /// [`set_current`](Context::set_current) or [`reset`](Context::reset).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use logwise::context::Context;
    ///
    /// let current = Context::current();
    /// println!("Current task: {}", current.task_id());
    /// ```
    #[inline]
    pub fn current() -> Context {
        CONTEXT.with(|c|
            //safety: we don't let anyone get a mutable reference to this
            unsafe{&*c.as_ptr()}.clone())
    }

    /// Returns the task associated with this context.
    ///
    /// If this context defines its own task, returns it. Otherwise, recursively
    /// searches parent contexts until a task is found.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use logwise::context::Context;
    ///
    /// let ctx = Context::new_task(None, "my_task".to_string(), logwise::Level::Info);
    /// // Access task properties through the context
    /// let task = ctx.task();
    ///
    /// // Child context inherits parent's task
    /// let child = Context::from_parent(ctx.clone());
    /// // child.task() returns the same task as parent
    /// ```
    pub fn task(&self) -> &Task {
        if let Some(task) = &self.inner.define_task {
            task
        } else {
            self.inner
                .parent
                .as_ref()
                .expect("No parent context")
                .task()
        }
    }

    /// Creates a new context with its own task.
    ///
    /// This creates a new context that defines a new task. The task will automatically
    /// log its creation and completion (when dropped), and collect performance statistics.
    ///
    /// # Arguments
    ///
    /// * `parent` - Optional parent context. If `None`, creates a root context.
    /// * `label` - Human-readable label for the task.
    /// * `completion_level` - Log level used when the task finishes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use logwise::context::Context;
    ///
    /// // Create a root task
    /// let root = Context::new_task(None, "main_process".to_string(), logwise::Level::Info);
    ///
    /// // Create a child task
    /// let child = Context::new_task(
    ///     Some(root.clone()),
    ///     "subprocess".to_string(),
    ///     logwise::Level::Info,
    /// );
    /// ```
    #[inline]
    pub fn new_task(parent: Option<Context>, label: String, completion_level: Level) -> Context {
        let context_id = CONTEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self::new_task_internal(parent, label, completion_level, context_id)
    }
    #[inline]
    fn new_task_internal(
        parent: Option<Context>,
        label: String,
        completion_level: Level,
        context_id: u64,
    ) -> Context {
        Context {
            inner: Arc::new(ContextInner {
                parent,
                context_id,
                define_task: Some(Task::new(label, completion_level)),
                is_tracing: AtomicBool::new(false),
            }),
        }
    }

    /// Resets the thread's context to a new root context.
    ///
    /// This is useful for starting fresh in a thread, discarding any existing
    /// context hierarchy. The new context becomes the current context for the thread.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use logwise::context::Context;
    ///
    /// // Start fresh with a new root context
    /// Context::reset("new_session".to_string());
    ///
    /// // All subsequent logs will use this new context
    /// logwise::info_sync!("Starting new session");
    /// ```
    #[inline]
    pub fn reset(label: String) {
        let new_context = Context::new_task(None, label, Level::DebugInternal);
        new_context.set_current();
    }

    /// Creates a new context that inherits from a parent context.
    ///
    /// Unlike [`new_task`](Context::new_task), this does not create a new task.
    /// Instead, it inherits the task from the parent context. This is useful for
    /// creating logical scopes within the same task.
    ///
    /// The new context also inherits the parent's tracing state.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use logwise::context::Context;
    ///
    /// let task_ctx = Context::new_task(None, "operation".to_string(), logwise::Level::Info);
    /// task_ctx.clone().set_current();
    ///
    /// // Create a sub-context within the same task
    /// let sub_ctx = Context::from_parent(Context::current());
    /// sub_ctx.clone().set_current();
    ///
    /// // Both contexts share the same task
    /// assert_eq!(task_ctx.task_id(), sub_ctx.task_id());
    /// ```
    pub fn from_parent(context: Context) -> Context {
        let is_tracing = context.inner.is_tracing.load(Ordering::Relaxed);
        Context {
            inner: Arc::new(ContextInner {
                parent: Some(context),
                context_id: CONTEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                define_task: None,
                is_tracing: AtomicBool::new(is_tracing),
            }),
        }
    }

    /// Returns the task ID associated with this context.
    ///
    /// This ID uniquely identifies the task and appears in all log messages
    /// generated within this context.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use logwise::context::Context;
    ///
    /// let ctx = Context::new_task(None, "my_task".to_string(), logwise::Level::Info);
    /// let task_id = ctx.task_id();
    /// println!("Task ID: {}", task_id);
    /// ```
    #[inline]
    pub fn task_id(&self) -> TaskID {
        self.task().task_id
    }

    /// Checks if this specific context has tracing enabled.
    ///
    /// When tracing is enabled, trace-level log messages will be output.
    /// Tracing state is inherited by child contexts created with [`from_parent`](Context::from_parent).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use logwise::context::Context;
    ///
    /// let ctx = Context::current();
    /// if ctx.is_tracing() {
    ///     // Trace logging is enabled for this context
    /// }
    /// ```
    #[inline]
    pub fn is_tracing(&self) -> bool {
        self.inner.is_tracing.load(Ordering::Relaxed)
    }

    /// Checks if the current thread's context has tracing enabled.
    ///
    /// This is a convenience method that checks the tracing state of the
    /// current thread-local context without needing to call [`Context::current`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use logwise::context::Context;
    ///
    /// if Context::currently_tracing() {
    ///     // Trace logging is enabled
    ///     logwise::trace_sync!("Detailed debug information");
    /// }
    /// ```
    #[inline]
    pub fn currently_tracing() -> bool {
        CONTEXT.with(|c| {
            //safety: we don't let anyone get a mutable reference to this
            unsafe { &*c.as_ptr() }
                .inner
                .is_tracing
                .load(Ordering::Relaxed)
        })
    }

    /// Enables tracing for the current context and its future children.
    ///
    /// Once tracing is enabled, trace-level log messages will be output.
    /// This setting is inherited by child contexts created with [`from_parent`](Context::from_parent)
    /// after tracing is enabled.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use logwise::context::Context;
    ///
    /// // Enable detailed tracing
    /// Context::begin_trace();
    ///
    /// // Now trace messages will be visible
    /// logwise::trace_sync!("This will be logged");
    ///
    /// // Child contexts inherit the tracing state
    /// let child = Context::from_parent(Context::current());
    /// assert!(child.is_tracing());
    /// ```
    pub fn begin_trace() {
        Context::current()
            .inner
            .is_tracing
            .store(true, Ordering::Relaxed);
        logwise::trace_sync!("Begin trace");
    }

    /// Sets this context as the current thread-local context.
    ///
    /// This replaces the thread's current context with this one. All subsequent
    /// logging operations on this thread will use this context until it's changed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use logwise::context::Context;
    ///
    /// let new_ctx = Context::new_task(None, "new_task".to_string(), logwise::Level::Info);
    /// new_ctx.set_current();
    ///
    /// // All logs now use the new context
    /// logwise::info_sync!("Using new context");
    /// ```
    pub fn set_current(self) {
        CONTEXT.replace(self);
    }

    /// Returns the nesting level of this context in the hierarchy.
    ///
    /// The root context has a nesting level of 0, its children have level 1, etc.
    /// This is used internally to determine log indentation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use logwise::context::Context;
    ///
    /// let root = Context::new_task(None, "root".to_string(), logwise::Level::Info);
    /// assert_eq!(root.nesting_level(), 0);
    ///
    /// let child = Context::from_parent(root.clone());
    /// assert_eq!(child.nesting_level(), 1);
    ///
    /// let grandchild = Context::from_parent(child.clone());
    /// assert_eq!(grandchild.nesting_level(), 2);
    /// ```
    pub fn nesting_level(&self) -> usize {
        let mut level = 0;
        let mut current = self;
        while let Some(parent) = &current.inner.parent {
            level += 1;
            current = parent;
        }
        level
    }

    /// Returns the unique ID of this context.
    ///
    /// Context IDs can be used with [`Context::pop`] to restore a previous context.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use logwise::context::Context;
    ///
    /// let ctx = Context::from_parent(Context::current());
    /// let id = ctx.context_id();
    /// ctx.clone().set_current();
    ///
    /// // Later, pop back to the parent
    /// Context::pop(id);
    /// ```
    #[inline]
    pub fn context_id(&self) -> ContextID {
        ContextID(self.inner.context_id)
    }

    /// Pops contexts from the current thread's stack until reaching the specified context.
    ///
    /// This searches up the context hierarchy for a context with the given ID.
    /// When found, sets the current context to that context's parent.
    /// If the ID is not found in the current hierarchy, logs a warning.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use logwise::context::Context;
    ///
    /// Context::reset("root".to_string());
    ///
    /// let child = Context::from_parent(Context::current());
    /// let child_id = child.context_id();
    /// child.clone().set_current();
    ///
    /// let grandchild = Context::from_parent(Context::current());
    /// grandchild.set_current();
    ///
    /// // Pop back to root (child's parent)
    /// Context::pop(child_id);
    /// ```
    pub fn pop(id: ContextID) {
        let mut current = Context::current();
        loop {
            if current.context_id() == id {
                let parent = current.inner.parent.clone().expect("No parent context");
                CONTEXT.replace(parent);
                return;
            }
            match current.inner.parent.as_ref() {
                None => {
                    logwise::warn_sync!(
                        "Tried to pop context with ID {id}, but it was not found in the current context chain.",
                        id = id.0
                    );
                    return;
                }
                Some(ctx) => current = ctx.clone(),
            }
        }
    }

    /// Internal: Writes the context prelude to a log record.
    ///
    /// This method is used internally by the logging macros to add context
    /// information (tracing marker, indentation, task ID) to log messages.
    ///
    /// # Note
    ///
    /// This is an implementation detail of the logging system and should not
    /// be called directly by users.
    #[doc(hidden)]
    #[inline]
    pub fn _log_prelude(&self, record: &mut crate::log_record::LogRecord) {
        let prefix = if self.is_tracing() { "T" } else { " " };
        record.log(prefix);
        for _ in 0..self.nesting_level() {
            record.log(" ");
        }
        record.log_owned(format!("{} ", self.task_id()));
    }

    /// Internal: Adds a performance interval to the current task's statistics.
    ///
    /// This method is used internally by the perfwarn system to accumulate
    /// performance statistics that are logged when the task completes.
    ///
    /// # Note
    ///
    /// This is an implementation detail of the logging system and should not
    /// be called directly by users.
    #[doc(hidden)]
    #[inline]
    pub fn _add_task_interval(&self, key: &'static str, duration: crate::sys::Duration) {
        self.task().add_task_interval(key, duration);
    }
}

/// A [`Future`] wrapper that preserves context across async executor boundaries.
///
/// Many async executors don't preserve thread-local state between poll calls,
/// which can cause context loss in async code. `ApplyContext` solves this by
/// saving and restoring the context around each poll.
///
/// # Use Cases
///
/// - Working with executors that use thread pools
/// - Spawning tasks that need to maintain parent context
/// - Ensuring consistent logging context in async code
///
/// # Examples
///
/// ```rust
/// use logwise::context::{Context, ApplyContext};
///
/// async fn process_data() {
///     logwise::info_sync!("Processing data");
/// }
///
/// # async fn example() {
/// // Create a context for this operation
/// let ctx = Context::new_task(None, "data_processor".to_string(), logwise::Level::Info);
///
/// // Wrap the future to preserve context
/// let future = ApplyContext::new(ctx, process_data());
///
/// // The context will be active during all poll calls
/// future.await;
/// # }
/// ```
///
/// # Implementation Details
///
/// `ApplyContext` implements [`Future`] by:
/// 1. Saving the current thread-local context
/// 2. Setting its wrapped context as current
/// 3. Polling the inner future
/// 4. Restoring the original context
///
/// This ensures the wrapped future always sees the correct context, regardless
/// of which thread or executor polls it.
pub struct ApplyContext<F>(Context, F);

impl<F> ApplyContext<F> {
    /// Creates a new `ApplyContext` wrapper.
    ///
    /// # Arguments
    ///
    /// * `context` - The context to apply during polling
    /// * `f` - The future to wrap
    ///
    /// # Examples
    ///
    /// ```rust
    /// use logwise::context::{Context, ApplyContext};
    /// use std::future::Future;
    ///
    /// async fn my_task() -> i32 {
    ///     logwise::info_sync!("Running task");
    ///     42
    /// }
    ///
    /// # async fn example() {
    /// let ctx = Context::new_task(None, "wrapped_task".to_string(), logwise::Level::Info);
    /// let wrapped = ApplyContext::new(ctx, my_task());
    /// let result = wrapped.await;
    /// assert_eq!(result, 42);
    /// # }
    /// ```
    pub fn new(context: Context, f: F) -> Self {
        Self(context, f)
    }
}

impl<F> Future for ApplyContext<F>
where
    F: Future,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let (context, fut) = unsafe {
            let d = self.get_unchecked_mut();
            (d.0.clone(), Pin::new_unchecked(&mut d.1))
        };
        let prior_context = Context::current();
        context.set_current();
        let r = fut.poll(cx);
        prior_context.set_current();
        r
    }
}

#[cfg(test)]
mod tests {
    use super::Level;
    use super::{Context, Task, TaskID};
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_new_context() {
        Context::reset("test_new_context".to_string());
        let port_context = Context::current();
        let next_context = Context::from_parent(port_context);
        let next_context_id = next_context.context_id();
        next_context.set_current();

        Context::pop(next_context_id);
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_context_equality() {
        Context::reset("test_context_equality".to_string());
        let context1 = Context::current();
        let context2 = context1.clone();
        let context3 = Context::new_task(None, "different_task".to_string(), Level::Info);

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

        Context::reset("test_context_hash".to_string());
        let context1 = Context::current();
        let context2 = context1.clone();
        let context3 = Context::new_task(None, "different_task".to_string(), Level::Info);

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
        Context::reset("root_task".to_string());
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
        let task_context = Context::new_task(
            Some(child_context.clone()),
            "child_task".to_string(),
            Level::Info,
        );
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
        Context::reset("test_as_ref".to_string());
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

        let new_task_context =
            Context::new_task(Some(context.clone()), "new_task".to_string(), Level::Info);
        let new_task_ref: &Task = new_task_context.as_ref();

        // New task should have different ID and label
        assert_ne!(new_task_ref.task_id, context.task_id());
        assert_eq!(new_task_ref.label, "new_task");
    }
}
