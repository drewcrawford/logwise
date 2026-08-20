// SPDX-License-Identifier: MIT OR Apache-2.0

//! Best-effort ingress for text produced outside logwise.
//!
//! These adapters do not change the portable observability contract: first-
//! party `logwise` events are the only input with consistent native/wasm
//! behavior, structured fields, context, and privacy policy. Foreign text is
//! always opaque, local-only, and excluded from remote sinks.

use std::panic;
use std::sync::Arc;

/// The producer of an opaque foreign text record.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ForeignOrigin {
    RustStdout,
    RustStderr,
    NightlyRustPrint,
    PanicHook,
    JsConsoleLog,
    JsConsoleDebug,
    JsConsoleInfo,
    JsConsoleWarn,
    JsConsoleError,
}

impl ForeignOrigin {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RustStdout => "rust.stdout",
            Self::RustStderr => "rust.stderr",
            Self::NightlyRustPrint => "rust.print.nightly",
            Self::PanicHook => "rust.panic",
            Self::JsConsoleLog => "js.console.log",
            Self::JsConsoleDebug => "js.console.debug",
            Self::JsConsoleInfo => "js.console.info",
            Self::JsConsoleWarn => "js.console.warn",
            Self::JsConsoleError => "js.console.error",
        }
    }
}

/// Imports one opaque local-only text record.
///
/// The `AdHocText` kind prevents the standard runtime from routing this event
/// to a remote sink even if a broad filter matches `foreign.text`.
pub fn foreign_text(origin: ForeignOrigin, text: &str) {
    logwise::__logwise_structured!(
        None;
        diagnostic,
        debug,
        AdHocText,
        "foreign.text",
        origin = local(origin.as_str()),
        text = local(text),
    );
}

type PanicHook = Box<dyn Fn(&panic::PanicHookInfo<'_>) + Send + Sync + 'static>;

/// The hook displaced by [`install_panic_hook`].
///
/// Hook replacement is process-global. Restoration is explicit rather than a
/// `Drop` side effect because another library may have installed a newer hook
/// in the meantime; applications must coordinate hook ownership.
pub struct PanicHookRegistration {
    previous: Option<PanicHook>,
}

impl std::fmt::Debug for PanicHookRegistration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PanicHookRegistration")
            .field("has_previous", &self.previous.is_some())
            .finish()
    }
}

impl PanicHookRegistration {
    /// Restores the hook that was active at installation time.
    pub fn restore(mut self) {
        if let Some(previous) = self.previous.take() {
            panic::set_hook(previous);
        }
    }
}

/// Installs a process-wide panic ingress before the previously registered hook.
pub fn install_panic_hook() -> PanicHookRegistration {
    let previous: Arc<dyn Fn(&panic::PanicHookInfo<'_>) + Send + Sync + 'static> =
        panic::take_hook().into();
    let chained = previous.clone();
    panic::set_hook(Box::new(move |info| {
        let rendered = info.to_string();
        foreign_text(ForeignOrigin::PanicHook, &rendered);
        chained(info);
    }));
    PanicHookRegistration {
        previous: Some(Box::new(move |info| previous(info))),
    }
}

/// Captures Rust's compiler-internal, thread-local print stream while `work`
/// runs, imports it as local-only text, then restores the prior capture.
///
/// This API exists only with `foreign-nightly-rust-print` and requires nightly
/// Rust's unstable `internal_output_capture` feature. It is intended for test
/// harness integration, not as a stable application interception mechanism.
#[cfg(feature = "foreign-nightly-rust-print")]
pub fn capture_nightly_rust_print<T>(work: impl FnOnce() -> T) -> T {
    use std::sync::{Arc, Mutex};

    let capture = Arc::new(Mutex::new(Vec::new()));
    let previous = std::io::set_output_capture(Some(capture.clone()));
    let result = panic::catch_unwind(panic::AssertUnwindSafe(work));
    std::io::set_output_capture(previous);
    let bytes = capture.lock().unwrap().clone();
    if !bytes.is_empty() {
        let text = String::from_utf8_lossy(&bytes);
        foreign_text(ForeignOrigin::NightlyRustPrint, &text);
    }
    match result {
        Ok(value) => value,
        Err(payload) => panic::resume_unwind(payload),
    }
}

#[cfg(unix)]
mod native_fd {
    use std::fs::File;
    use std::io::{self, Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::thread::JoinHandle;

    use super::{ForeignOrigin, foreign_text};

    type RawFd = std::os::raw::c_int;

    unsafe extern "C" {
        fn pipe(descriptors: *mut RawFd) -> RawFd;
        fn dup(descriptor: RawFd) -> RawFd;
        fn dup2(source: RawFd, destination: RawFd) -> RawFd;
    }

    /// A process-global native file descriptor accepted by the capture adapter.
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub enum NativeFd {
        Stdout,
        Stderr,
    }

    impl NativeFd {
        const fn raw(self) -> RawFd {
            match self {
                Self::Stdout => 1,
                Self::Stderr => 2,
            }
        }

        const fn origin(self) -> ForeignOrigin {
            match self {
                Self::Stdout => ForeignOrigin::RustStdout,
                Self::Stderr => ForeignOrigin::RustStderr,
            }
        }

        fn flush(self) -> io::Result<()> {
            match self {
                Self::Stdout => io::stdout().flush(),
                Self::Stderr => io::stderr().flush(),
            }
        }
    }

    /// An active best-effort native FD redirection.
    ///
    /// Redirection affects the whole process, races with unrelated writers and
    /// parallel tests, and does not intercept libtest's thread-local Rust print
    /// capture. It carries no logical context or field-level privacy. Finish it
    /// promptly and never treat it as equivalent to first-party instrumentation.
    pub struct NativeFdCapture {
        target: NativeFd,
        saved: Option<File>,
        reader: Option<JoinHandle<io::Result<Vec<u8>>>>,
    }

    impl std::fmt::Debug for NativeFdCapture {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("NativeFdCapture")
                .field("target", &self.target)
                .field("active", &self.reader.is_some())
                .finish_non_exhaustive()
        }
    }

    impl NativeFdCapture {
        /// Redirects the selected process file descriptor into a pipe.
        pub fn start(target: NativeFd) -> io::Result<Self> {
            target.flush()?;
            let mut descriptors = [0; 2];
            // SAFETY: `descriptors` points to two writable C integers.
            if unsafe { pipe(descriptors.as_mut_ptr()) } == -1 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: successful `pipe` returned two owned descriptors.
            let read = unsafe { File::from_raw_fd(descriptors[0]) };
            // SAFETY: successful `pipe` returned two owned descriptors.
            let write = unsafe { File::from_raw_fd(descriptors[1]) };
            // SAFETY: `target.raw()` names an open process standard stream.
            let saved_raw = unsafe { dup(target.raw()) };
            if saved_raw == -1 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: `dup` returned a new descriptor owned by this adapter.
            let saved = unsafe { File::from_raw_fd(saved_raw) };
            // SAFETY: both descriptors are open; dup2 atomically replaces the
            // target while retaining the independent `write` descriptor.
            if unsafe { dup2(write.as_raw_fd(), target.raw()) } == -1 {
                return Err(io::Error::last_os_error());
            }
            drop(write);
            let reader = std::thread::spawn(move || {
                let mut read = read;
                let mut bytes = Vec::new();
                read.read_to_end(&mut bytes)?;
                Ok(bytes)
            });
            Ok(Self {
                target,
                saved: Some(saved),
                reader: Some(reader),
            })
        }

        /// Restores the descriptor, imports the captured bytes as one local-only
        /// record, and returns the same lossy UTF-8 text to the caller.
        pub fn finish(mut self) -> io::Result<String> {
            self.restore()?;
            let bytes = self.join_reader()?;
            let text = String::from_utf8_lossy(&bytes).into_owned();
            if !text.is_empty() {
                foreign_text(self.target.origin(), &text);
            }
            Ok(text)
        }

        fn restore(&mut self) -> io::Result<()> {
            self.target.flush()?;
            let Some(saved) = self.saved.take() else {
                return Ok(());
            };
            // SAFETY: `saved` and the standard target are valid descriptors.
            if unsafe { dup2(saved.as_raw_fd(), self.target.raw()) } == -1 {
                self.saved = Some(saved);
                return Err(io::Error::last_os_error());
            }
            drop(saved);
            Ok(())
        }

        fn join_reader(&mut self) -> io::Result<Vec<u8>> {
            self.reader
                .take()
                .map(|reader| {
                    reader
                        .join()
                        .map_err(|_| io::Error::other("logwise native FD reader thread panicked"))?
                })
                .transpose()
                .map(Option::unwrap_or_default)
        }
    }

    impl Drop for NativeFdCapture {
        fn drop(&mut self) {
            let _ = self.restore();
            let _ = self.join_reader();
        }
    }
}

#[cfg(unix)]
pub use native_fd::{NativeFd, NativeFdCapture};
