// SPDX-License-Identifier: MIT OR Apache-2.0

//! Core Context implementation.

use crate::Level;
use std::cell::{Cell, OnceCell};
use std::fmt::Display;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::task::{Task, TaskID};

pub(crate) static CONTEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Unique identifier for a context.
///
/// Each context gets a unique ID that can be used to identify and pop specific
/// contexts from the context stack.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContextID(pub(crate) u64);

/// Internal context data.
///
/// This structure holds the actual context state, wrapped in an Arc for cheap cloning.
#[derive(Debug)]
pub(crate) struct ContextInner {
    pub(crate) parent: Option<Context>,
    pub(crate) context_id: u64,
    /// If Some, this context defines a new task. If None, inherits task from parent.
    pub(crate) define_task: Option<Task>,
    /// Whether this context is currently tracing.
    pub(crate) is_tracing: AtomicBool,
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
/// let root = Context::new_task(None, "root_operation".to_string(), logwise::Level::Info, true);
/// root.clone().set_current();
///
/// // Create a child context with a new task
/// let child = Context::new_task(
///     Some(Context::current()),
///     "child_operation".to_string(),
///     logwise::Level::Info,
///     true,
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
    pub(crate) inner: Arc<ContextInner>,
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
    pub(crate) static CONTEXT: OnceCell<Cell<Context>> = const { OnceCell::new() };
}

/// Lazily initializes and returns the thread-local context cell.
fn get_or_init_context(once: &OnceCell<Cell<Context>>) -> &Cell<Context> {
    once.get_or_init(|| {
        Cell::new(Context::new_task_internal(
            None,
            String::new(),
            Level::DebugInternal,
            false,
            0,
        ))
    })
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
        CONTEXT.with(|once| {
            let c = get_or_init_context(once);
            //safety: we don't let anyone get a mutable reference to this
            unsafe { &*c.as_ptr() }.clone()
        })
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
    /// let ctx = Context::new_task(None, "my_task".to_string(), logwise::Level::Info, true);
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
    /// let root = Context::new_task(None, "main_process".to_string(), logwise::Level::Info, true);
    ///
    /// // Create a child task
    /// let child = Context::new_task(
    ///     Some(root.clone()),
    ///     "subprocess".to_string(),
    ///     logwise::Level::Info,
    ///     true,
    /// );
    /// ```
    #[inline]
    pub fn new_task(
        parent: Option<Context>,
        label: String,
        completion_level: Level,
        should_log_completion: bool,
    ) -> Context {
        let context_id = CONTEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self::new_task_internal(
            parent,
            label,
            completion_level,
            should_log_completion,
            context_id,
        )
    }

    #[inline]
    pub(crate) fn new_task_internal(
        parent: Option<Context>,
        label: String,
        completion_level: Level,
        should_log_completion: bool,
        context_id: u64,
    ) -> Context {
        Context {
            inner: Arc::new(ContextInner {
                parent,
                context_id,
                define_task: Some(Task::new(label, completion_level, should_log_completion)),
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
    /// logwise::declare_logging_domain!();
    /// # fn main() {
    /// use logwise::context::Context;
    ///
    /// // Start fresh with a new root context
    /// Context::reset("new_session".to_string());
    ///
    /// // All subsequent logs will use this new context
    /// logwise::info_sync!("Starting new session");
    /// # }
    /// ```
    #[inline]
    pub fn reset(label: String) {
        let new_context = Context::new_task(
            None,
            label,
            Level::DebugInternal,
            crate::log_enabled!(
                Level::DebugInternal,
                crate::__CALL_LOGWISE_DECLARE_LOGGING_DOMAIN
            ),
        );
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
    /// let task_ctx = Context::new_task(None, "operation".to_string(), logwise::Level::Info, true);
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
    /// let ctx = Context::new_task(None, "my_task".to_string(), logwise::Level::Info, true);
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
    /// logwise::declare_logging_domain!();
    /// # fn main() {
    /// use logwise::context::Context;
    ///
    /// if Context::currently_tracing() {
    ///     // Trace logging is enabled
    ///     logwise::trace_sync!("Detailed debug information");
    /// }
    /// # }
    /// ```
    #[inline]
    pub fn currently_tracing() -> bool {
        CONTEXT
            .try_with(|once| {
                // If not initialized yet, we're not tracing
                let Some(c) = once.get() else {
                    return false;
                };
                //safety: we don't let anyone get a mutable reference to this
                unsafe { &*c.as_ptr() }
                    .inner
                    .is_tracing
                    .load(Ordering::Relaxed)
            })
            .unwrap_or(false)
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
    /// logwise::declare_logging_domain!();
    /// # fn main() {
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
    /// # }
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
    /// logwise::declare_logging_domain!();
    /// # fn main() {
    /// use logwise::context::Context;
    ///
    /// let new_ctx = Context::new_task(None, "new_task".to_string(), logwise::Level::Info, true);
    /// new_ctx.set_current();
    ///
    /// // All logs now use the new context
    /// logwise::info_sync!("Using new context");
    /// # }
    /// ```
    pub fn set_current(self) {
        CONTEXT.with(|once| {
            get_or_init_context(once).replace(self);
        });
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
    /// let root = Context::new_task(None, "root".to_string(), logwise::Level::Info, true);
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
                CONTEXT.with(|once| {
                    get_or_init_context(once).replace(parent);
                });
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

    /// Internal: Adds a conditional performance interval to the current task's statistics.
    #[doc(hidden)]
    #[inline]
    pub fn _add_task_interval_if(
        &self,
        key: &'static str,
        duration: crate::sys::Duration,
        threshold: crate::sys::Duration,
    ) {
        self.task().add_task_interval_if(key, duration, threshold);
    }
}
