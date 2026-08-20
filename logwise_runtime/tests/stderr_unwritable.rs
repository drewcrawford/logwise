// SPDX-License-Identifier: MIT OR Apache-2.0

//! Logging to a stderr that cannot be written must not panic.
//!
//! This is native-unix-only, and deliberately so: it replaces the process's
//! real file descriptor 2 with the write end of a pipe whose reader is already
//! closed, which is the cheapest way to make every `write` return `EPIPE`. A
//! browser has neither file descriptors nor a stderr to break, so there is
//! nothing for the wasm32 target to run here.
#![cfg(all(unix, not(target_arch = "wasm32")))]

use std::os::raw::c_int;
use std::panic::{self, AssertUnwindSafe};

use logwise_runtime::{Level, LogRecord, Logger, StdErrorLogger};

unsafe extern "C" {
    fn pipe(descriptors: *mut c_int) -> c_int;
    fn dup(descriptor: c_int) -> c_int;
    fn dup2(source: c_int, destination: c_int) -> c_int;
    fn close(descriptor: c_int) -> c_int;
}

const STDERR: c_int = 2;

/// Redirects fd 2 at a pipe with no reader, runs `work`, then restores fd 2.
fn with_broken_stderr<R>(work: impl FnOnce() -> R) -> R {
    let mut descriptors = [0 as c_int; 2];
    // SAFETY: `descriptors` points at two writable C integers.
    assert_eq!(unsafe { pipe(descriptors.as_mut_ptr()) }, 0, "pipe");
    // SAFETY: fd 2 is open for the life of the process.
    let saved = unsafe { dup(STDERR) };
    assert!(saved >= 0, "dup stderr");
    // Closing the read end is what turns every later write into EPIPE. Rust
    // ignores SIGPIPE at startup, so the write reports the error rather than
    // killing the process outright.
    // SAFETY: both descriptors came from the successful `pipe` above.
    assert_eq!(unsafe { close(descriptors[0]) }, 0, "close read end");
    // SAFETY: the write end is open and fd 2 is a valid destination.
    assert!(
        unsafe { dup2(descriptors[1], STDERR) } >= 0,
        "redirect stderr"
    );

    let outcome = panic::catch_unwind(AssertUnwindSafe(work));

    // SAFETY: `saved` is the descriptor duplicated from fd 2 above.
    assert!(unsafe { dup2(saved, STDERR) } >= 0, "restore stderr");
    // SAFETY: both descriptors are still owned by this function.
    unsafe {
        close(saved);
        close(descriptors[1]);
    }

    match outcome {
        Ok(value) => value,
        Err(payload) => panic::resume_unwind(payload),
    }
}

#[test]
fn a_broken_stderr_loses_the_line_instead_of_the_process() {
    let mut record = LogRecord::new(Level::Error);
    record.log("stderr is a closed pipe here");

    // The panic hook itself writes to stderr, so silence it for the window in
    // which stderr is the broken pipe.
    let previous = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let logged = with_broken_stderr(|| {
        panic::catch_unwind(AssertUnwindSafe(|| {
            StdErrorLogger::new().finish_log_record(record.clone());
        }))
    });
    panic::set_hook(previous);

    assert!(
        logged.is_ok(),
        "writing a record to an unwritable stderr panicked"
    );

    // Still usable once stderr comes back.
    StdErrorLogger::new().finish_log_record(record);
}
