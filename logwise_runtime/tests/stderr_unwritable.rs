// SPDX-License-Identifier: MIT OR Apache-2.0

//! Logging to a stderr that cannot be written must not panic.
//!
//! This is native-unix-only, and deliberately so: it replaces the process's
//! real file descriptor 2 with the write end of a pipe whose reader is already
//! closed, which is the cheapest way to make every `write` return `EPIPE`. A
//! browser has neither file descriptors nor a stderr to break, so there is
//! nothing for the wasm32 target to run here.
#![cfg(all(unix, not(target_arch = "wasm32")))]

use std::fs::File;
use std::io::Read;
use std::os::fd::FromRawFd;
use std::os::raw::c_int;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex, MutexGuard};

use logwise::{Class, ContextToken, Kind, Metadata, Severity};
use logwise_runtime::{
    ConsoleSink, EventSink, InMemoryLogger, Level, LogRecord, Logger, ProjectedEvent,
    StdErrorLogger,
};

unsafe extern "C" {
    fn pipe(descriptors: *mut c_int) -> c_int;
    fn dup(descriptor: c_int) -> c_int;
    fn dup2(source: c_int, destination: c_int) -> c_int;
    fn close(descriptor: c_int) -> c_int;
}

const STDERR: c_int = 2;

/// fd 2 and the panic hook are both process-global, so only one case at a time
/// may be holding them hostage.
static SERIAL: Mutex<()> = Mutex::new(());

fn serialized() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|error| error.into_inner())
}

/// Redirects fd 2 at a pipe with no reader, runs `work`, then restores fd 2.
///
/// The panic hook writes to stderr too, so it is silenced for the same window.
fn with_broken_stderr<R>(work: impl FnOnce() -> R) -> R {
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
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

    panic::set_hook(previous_hook);

    match outcome {
        Ok(value) => value,
        Err(payload) => panic::resume_unwind(payload),
    }
}

/// Redirects fd 2 into a pipe this function drains, runs `work`, then restores
/// fd 2 and returns whatever reached the descriptor.
///
/// This is the half of the contract libtest hides: `eprintln!` is redirected
/// into the harness's per-thread capture buffer and never reaches fd 2 at all,
/// so a sink that writes through it neither reaches a redirected stderr nor
/// notices when the write fails.
fn stderr_output_of<R>(work: impl FnOnce() -> R) -> (R, String) {
    let mut descriptors = [0 as c_int; 2];
    // SAFETY: `descriptors` points at two writable C integers.
    assert_eq!(unsafe { pipe(descriptors.as_mut_ptr()) }, 0, "pipe");
    // SAFETY: fd 2 is open for the life of the process.
    let saved = unsafe { dup(STDERR) };
    assert!(saved >= 0, "dup stderr");
    // SAFETY: the write end is open and fd 2 is a valid destination.
    assert!(
        unsafe { dup2(descriptors[1], STDERR) } >= 0,
        "redirect stderr"
    );
    // SAFETY: fd 2 now holds the only copy of the write end this test needs.
    unsafe { close(descriptors[1]) };

    let value = work();

    // SAFETY: `saved` is the descriptor duplicated from fd 2 above. Restoring
    // closes the last write end, which is what lets the read below see EOF.
    assert!(unsafe { dup2(saved, STDERR) } >= 0, "restore stderr");
    // SAFETY: `saved` is owned here and no longer needed.
    unsafe { close(saved) };

    // SAFETY: the read end came from the successful `pipe` above and is
    // handed over exactly once.
    let mut reader = unsafe { File::from_raw_fd(descriptors[0]) };
    let mut written = String::new();
    reader.read_to_string(&mut written).expect("read pipe");
    (value, written)
}

static METADATA: Metadata = Metadata {
    event_name: "logwise_runtime.test.unwritable",
    package: "logwise_runtime",
    target: "stderr_unwritable",
    module: "stderr_unwritable",
    domain: None,
    severity: Severity::Warn,
    class: Class::Diagnostic,
    kind: Kind::Event,
    location: None,
    fields: &[],
};

#[test]
fn a_broken_stderr_loses_the_line_instead_of_the_process() {
    let _serial = serialized();
    let mut record = LogRecord::new(Level::Error);
    record.log("stderr is a closed pipe here");

    let logged = with_broken_stderr(|| {
        panic::catch_unwind(AssertUnwindSafe(|| {
            StdErrorLogger::new().finish_log_record(record.clone());
        }))
    });

    assert!(
        logged.is_ok(),
        "writing a record to an unwritable stderr panicked"
    );

    // Still usable once stderr comes back.
    StdErrorLogger::new().finish_log_record(record);
}

fn console_event() {
    ConsoleSink.emit(ProjectedEvent {
        metadata: &METADATA,
        context: ContextToken::NONE,
        fields: Vec::new(),
        message: None,
        omitted_fields: 0,
    });
}

#[test]
fn console_output_goes_to_the_real_stderr_and_survives_it_breaking() {
    let _serial = serialized();
    let logger = Arc::new(InMemoryLogger::new());
    let mut record = LogRecord::new(Level::Error);
    record.log("drained to the console");
    logger.finish_log_record(record);

    let ((), written) = stderr_output_of(|| {
        console_event();
        logger.drain_to_console();
    });
    assert!(
        written.contains("logwise_runtime.test.unwritable"),
        "the console sink did not reach fd 2; it wrote {written:?}"
    );
    assert!(
        written.contains("drained to the console"),
        "the drain did not reach fd 2; it wrote {written:?}"
    );

    // And the same two writes against a descriptor that refuses them.
    let mut record = LogRecord::new(Level::Error);
    record.log("drained into a closed pipe");
    logger.finish_log_record(record);
    let outcome = with_broken_stderr(|| {
        panic::catch_unwind(AssertUnwindSafe(|| {
            console_event();
            // The drain holds the buffer lock across the write, so a panic here
            // would poison it and take every later record with it.
            logger.drain_to_console();
        }))
    });
    assert!(
        outcome.is_ok(),
        "an unwritable stderr panicked a sink or the in-memory logger"
    );

    let mut record = LogRecord::new(Level::Error);
    record.log("captured after stderr came back");
    logger.finish_log_record(record);
    assert_eq!(
        logger.drain_logs(),
        "captured after stderr came back",
        "the logger kept working, and the drained batches really were drained"
    );
}
