// SPDX-License-Identifier: MIT OR Apache-2.0

//! Logging from a destructor that runs at thread exit must not take the
//! process down.
//!
//! Thread-local destructors run in an unspecified order, so a value that logs
//! on drop can easily outlive logwise's own thread-local context. Touching a
//! destroyed thread-local panics, and a panic escaping a thread-local
//! destructor is a `fatal runtime error: thread local panicked on drop` --
//! an abort, not a catchable failure, which is why this test needs its own
//! binary: it would have taken every other test in the file with it.
//!
//! Native-only: `std::thread::spawn` is unsupported on wasm32, and thread-local
//! teardown is what is under test here rather than anything portable.
#![cfg(not(target_arch = "wasm32"))]

use std::sync::atomic::{AtomicBool, Ordering};

logwise_runtime::declare_logging_domain!();

static DROPPED: AtomicBool = AtomicBool::new(false);

struct LogsOnDrop;

impl Drop for LogsOnDrop {
    fn drop(&mut self) {
        logwise_runtime::warn_sync!("logwise logged from a thread-local destructor");
        // A perfwarn interval reaches for the context twice: once at
        // construction and once on its own drop.
        drop(logwise_runtime::perfwarn_begin!("thread exit interval"));
        DROPPED.store(true, Ordering::Release);
    }
}

thread_local! {
    static LOGS_ON_DROP: LogsOnDrop = const { LogsOnDrop };
}

#[test]
fn a_destructor_may_log_after_the_context_thread_local_is_gone() {
    std::thread::spawn(|| {
        // Registering this destructor first, and only then letting a log call
        // register logwise's context thread-local, is what puts the context
        // cell ahead of it in the teardown order.
        LOGS_ON_DROP.with(|_| ());
        logwise_runtime::warn_sync!("worker is running normally");
    })
    .join()
    .expect("worker thread");

    assert!(
        DROPPED.load(Ordering::Acquire),
        "the destructor did not finish"
    );

    // The main thread's own context is untouched by any of that.
    logwise_runtime::warn_sync!("main thread still logs");
}
