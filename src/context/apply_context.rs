// SPDX-License-Identifier: MIT OR Apache-2.0

//! Async context preservation.

use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

use super::context_impl::Context;

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
/// let ctx = Context::new_task(None, "data_processor".to_string(), logwise::Level::Info, true);
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
    /// let ctx = Context::new_task(None, "wrapped_task".to_string(), logwise::Level::Info, true);
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
