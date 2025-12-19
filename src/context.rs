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
//!     true,
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
//! logwise::declare_logging_domain!();
//! # fn main() {
//! # use logwise::context::Context;
//! # Context::reset("test".to_string());
//! // Performance intervals are automatically added to the current task
//! // When using the perfwarn! macro:
//! logwise::perfwarn!("expensive_operation", {
//!     // This duration is automatically tracked in the current context's task
//!     std::thread::sleep(std::time::Duration::from_millis(100));
//! });
//! // Statistics are logged when the task is dropped
//! # }
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
//! let ctx = Context::new_task(None, "async_task".to_string(), logwise::Level::Info, true);
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

mod apply_context;
mod context_impl;
mod task;

#[cfg(test)]
mod tests;

// Re-export public types
pub use apply_context::ApplyContext;
pub use context_impl::{Context, ContextID};
pub use task::{Task, TaskID};
