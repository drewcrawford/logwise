# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Call sites now have a small path and a durable path.** `log!` provides private-by-default Rust formatting that stays available in optimized builds, while `event!`, `span!`, `counter!`, and `measurement!` attach stable metadata and independently gated privacy/detail fields. Crates can remove a site completely with their own `#[cfg]`, and no crate-root domain declaration is required.

### Changed

- **Logwise is now split at the dependency boundary.** The `logwise` package is a `no_std`, no-allocation-by-default facade with no dependencies; the previous implementation lives in `logwise_runtime` while it is migrated onto the new contract. A no-alloc fixture and dependency-tree gate keep the facade tiny on native and wasm targets.

- **The new facade dispatches borrowed observations synchronously.** Call sites now carry static schema metadata, privacy and detail policy, opaque context tokens, typed borrowed values, and a generation-cached interest mask. With no runtime installed, dispatch is a no-op and field work is skipped; runtimes install one process dispatcher and copy only what they retain.

- **Context is durable across executors without living in the facade.** Tasks now own fixed-size copyable context tokens, while scoped non-send guards let the runtime install them only around a poll and restore the prior thread/worker-local token afterward. The runtime records distinct parents and links, supports expiring descendant-targeted interest, and owns wall-time, active-time, and wake-latency clocks. Performance-warning spans retain their originating context and report threshold violations when dropped.

- **Sinks see capabilities, not promises.** The runtime unions interest across hierarchical metadata/context selectors, then constructs separate `ProjectedEvent` views for remote-safe, retained-local, and explicitly trusted ephemeral sinks. Remote code cannot receive local-only or secret values, retained sinks cannot receive secrets, opaque ad-hoc text never reaches remote views, and TTL activation distinguishes unavailable targets, omitted instrumentation, and unknown selectors.

- **Recording and durability are separate operations.** Console, bounded in-memory, structured-writer, and queued async sinks now live behind the runtime dispatcher. Queues report accepted, dropped, overwritten, truncated, and failed records; explicit future/blocking barriers flush through a captured sequence, while ordinary call sites never await. Sink callbacks run outside configuration locks, recursive logging is dropped and counted, and unwind-capable builds isolate a panicking sink from the rest of the fan-out.

- **The flight recorder remembers structure, not prose.** Its fixed-slot shards keep recent core events behind monotonic cursors without making writers wait. Reads explicitly report overwrites, contention drops, truncation, omissions, and temporarily busy shards; local queries retain local-only fields, while remote queries receive a fresh support-safe projection and never an opaque ad-hoc message.

## [0.6.1] - 2026-08-17

### Fixed

- **The declared MSRV was wrong, and 0.6.0 shipped with it.** `rust-version` said 1.85.1, but `wasm_lite_std` sits in the shared `[dependencies]` table and requires 1.95.0, so that has been the real floor on every target since we adopted it. A build on 1.85–1.94 did not get a clean "requires rustc 1.95.0"; it got a compile error from inside `wasm_lite_std`, and cargo's MSRV-aware resolver would happily pick 0.6.0 for a toolchain that cannot build it. Now declared honestly.

- **The wasm32 test suite was not running. Any of it.** `.cargo/config.toml` still named `wasm-bindgen-test-runner` as the runner, but the tests moved to `#[wasm_lite_test]` some time ago, and those register in a custom `__wasm_lite_tests` section that runner does not read. So it found zero cases, printed `no tests to run!`, and exited 0 — a green wasm32 CI covering nothing at all. It now runs `wasm_lite run`, which finds them in a real browser. `--export=__stack_pointer` joins the link flags, because that is what `wasm_lite`'s worker bootstrap reads to place a worker's stack. CI no longer installs `wasm-bindgen-cli`: nothing in the wasm32 graph reaches wasm-bindgen any more, and the step's own comment already said it was load-bearing only for the old runner.

- **Ten more test files were native-only without saying so.** With the runner fixed, the suites that had never been ported stood out: a bare `#[test]` compiles for wasm32 and then registers nothing, so `brace_escaping`, `context_ids`, `debuginternal`, `interval_context`, `macro_hygiene_and_escapes`, `macro_refactor`, `privacy_redaction`, `repeated_placeholder`, `unused_structured_fields` and `heartbeat_context` all built and ran zero cases there. They now carry the `cfg_attr` pair and run on both targets — `heartbeat_context` on a worker, since it sleeps. Three files stay native-only on purpose, and now say why in a comment: `macro_expressions` shells out to `cargo check` to assert on a proc-macro diagnostic, `global_logger_deadlock` re-enters itself as a subprocess with a `process::exit` watchdog, and `release_script` asserts on text `include_str!` already baked in at compile time.

- **`src/global_logger.rs` had the same gap, and one of its tests never could have passed.** Its three unit tests were the only bare `#[test]`s left in `src/`. Porting them surfaced a genuine incompatibility rather than a bookkeeping one: `test_thread_safety` called `std::thread::spawn`, which on wasm32-unknown-unknown returns `Unsupported` — `std::thread::sleep` works there, so the two are easy to assume equivalent. It now goes through the `wasm_lite_std` veneer, the way `inmemory_logger` and `heartbeat` already did, and runs on a worker that spawns a worker.

- **`tests/heartbeat.rs` did not even compile for wasm32.** It was the last user of `test_executors::async_test`, whose *published* expansion is `#[wasm_bindgen_test]` — and `wasm_bindgen_test` is not a dependency here, so the target failed to build. The failure was invisible behind the runner above, since a suite that runs nothing looks much like a suite that compiles.

- **`logwise_proc` was outside the workspace, so nothing in it was ever checked.** It is a path dependency, which is not the same as a workspace member: `--all` resolved to `logwise` alone and `cargo test -p logwise_proc` refused outright with "not a member of the workspace" — while `AGENTS.md` claimed the opposite. Adding it to `[workspace] members` turned on 24 doctests and a clippy pass that had never run, and **16 of the 24 doctests failed immediately**: every macro this crate exports was documented with an example missing `declare_logging_domain!()`, which logwise requires per crate. They were wrong in the published docs for as long as they have existed. The examples now carry the declaration and an explicit `fn main()`, which is what puts it at crate root — `debuginternal_async` had the declaration already and still failed for want of the `fn main()`. Clippy also had three genuine lints waiting in `parser.rs`, `perfwarn.rs` and `profile_attr.rs`.

- **A doctest that claimed what `lformat!` expands to now checks it.** It was fenced ```` ```ignore ````, so the "expands to approximately" call sequence was prose that nothing verified. The mock logger records its calls and the example asserts the exact sequence, so changing the expansion breaks the documentation describing it. The remaining six `ignore` fences in `parser.rs` were `//` comments rather than Rust — pseudo-code for `parse_key`, `parse_value` and `build_kvs`, which cannot be unit-tested at all because they take `proc_macro::TokenTree`, an API that panics outside a macro invocation. They are now fenced ```` ```text ````, which is what they are. No doctest in the workspace is skipped any more.

### Removed

- **The `test_executors` dev-dependency.** `#[wasm_lite_test]` covers what it was doing, so `tests/heartbeat.rs` now uses that directly — a sync body on a worker, matching `tests/perfwarn_if.rs`, with the async-only-so-wasm32-can-await scaffolding deleted. Three things go with it: the release-order knot (`test_executors` depends on `logwise`, so neither could go first), the `wasm-bindgen`/`web-sys` tree it pulled into the wasm32 test graph, and the second copy of `logwise` that came with it. The root crate now has no dev-dependencies at all.

## [0.6.0] - 2026-08-15

### Added

- Added `LogPrivacy`, giving trusted local logger implementations an explicit opt-in to full private diagnostics while keeping new destinations safely redacted by default.
- `Instant` is re-exported at the crate root alongside `Duration`. It appears in the signatures of public API such as `LogRecord::log_time_since` and `interval::PerfwarnInterval::new`, and on wasm32 it is not `std::time::Instant`, so callers had no way to name it without depending on `wasm_lite_std` themselves.

### Changed

- Moved the WebAssembly logging, clock, threading, and test infrastructure to `wasm_lite` and `wasm_lite_std`, removing the `wasm-bindgen`, `web-sys`, and related dependency stack.

### Fixed

- Custom loggers now receive privacy-redacted values by default, while explicitly local stderr and in-memory loggers retain full diagnostic detail. Private values no longer hitch an accidental ride to remote logging services.
- Replacing global loggers now destroys retired logger instances after releasing the configuration lock, preventing re-entrant logger cleanup from deadlocking the process.
- Performance and profiling intervals stay attached to the context where they began, even when guards move between threads or another context becomes current before they finish.
- Heartbeat warnings stay attached to the context where the heartbeat began instead of inheriting the watcher or drop thread's current context.
- Lazily created root contexts now receive genuinely unique IDs instead of colliding with the first public context.
- Logging macros now preserve valid Rust expressions such as casts instead of collapsing required token spacing.
- Logging macro key/value fields that are not interpolated into the message are appended in call-site order instead of disappearing quietly.
- WebAssembly error logging no longer contains a magic `DEBUGME` message that panics instead of writing the record.
- The native release script now builds logwise itself instead of trying to package files from an unrelated application. A small but important identity check for the release machinery.
- Unicode escapes in format strings are escapes again. `"\u{1F600}"` used to be read as a placeholder named `1F600` and refuse to compile; now it is just an emoji.
- Log values can contain generic argument lists. `HashMap::<u8, u8>::new()` was being chopped in half at the comma between the type parameters.
- Logging macros no longer reach out and grab call-site variables named `record`, `formatter`, `id`, `interval`, or `result`. Your names are yours again.
- Macro errors now say what is actually wrong. A missing key used to surface as a syntax error pointing into logwise's own generated code, burying the real "Key ... not found" message.
- `perfwarn!` reports a compile error on an empty invocation instead of panicking the compiler, and expands the measured block once instead of duplicating it into both branches of the level check.
- Escaped braces in log format strings now render literally, so `{{key}}` produces `{key}` instead of being treated as an interpolation.
- Performance-warning interval closing records now use the same `PERFWARN` label as their opening records.
- Idle heartbeat watchers now sleep until work arrives instead of waking four times per second.
- Contexts are restored after a wrapped future panics, and attempts to pop a root context now warn instead of panicking.
- Periodic in-memory log draining keeps at most one timer active, preventing repeated polls from accumulating sleeping threads.
- Internal spinlocks now require the correct thread-safety bound and always unlock when a protected closure panics.
- WebAssembly tests build and run on current nightly toolchains, including threaded tests and doctests in Chrome.
- The wasm32 gate compiles again on current nightlies. Nightly deprecated `Atomic::fetch_update` in favour of a `try_update` our MSRV does not have, and `-D warnings` turned that into a hard error before anything else got a chance to run.
- The wasm32 test binaries link again. The dev-cycle `[patch.crates-io]` block covered `logwise` but not `wasm_lite`, so we took it by path while `some_executor` took it from the registry — and since `wasm_lite` exports `#[no_mangle]` symbols, two copies in one graph is a `duplicate symbol` link failure rather than a silent duplicate.
- Dropped the dev-cycle patch for `continue`, which now builds against the published 0.1.3. That release left wasm-bindgen behind and takes `wasm_lite` only as a dev-dependency, so it no longer drags anything into our graph — one less sibling checkout to have cloned.
- `declare_logging_domain!()`'s documentation described a `logwise_internal` feature flag that it never consulted. It compares `CARGO_CRATE_NAME` against `module_path!()`; the docs now say so, and show how to gate on a feature if that is what you were after.
- `LogRecord::log_time_since` documented itself as measuring elapsed time. It stamps an already-captured instant's offset from process start, which is what every caller wants of it, and now what it claims.
- An argument with no `key =` in front of it is now a compile error naming the offender. `warn_sync!("finished", count)` reads like `format!`'s implicit capture, but logwise has no such thing; it used to be discarded in silence, taking every argument after it along with it.
- A `{key}` used more than once in a format string evaluates its value exactly once. `"attempt {n}, retrying {n}"` used to splice the expression in twice, so side effects ran twice and the two spots could disagree — and a value that gets moved failed to compile, pointing at generated code rather than at your call.

## [0.5.1] - 2026-02-14

### Changed

- Switched wasm thread support to `wasm_safe_thread` for safer cross-target behavior.

### Fixed

- Documentation polish and intra-doc cleanup to keep `cargo doc -D warnings` green.

## [0.5.0] - 2025-12-20

### Added

- **Mandatory logging level** - Sometimes you just *need* a message to show up, no matter what. The new `mandatory_sync!` and `mandatory_async!` macros ensure your logs break through even the quietest of configurations.
- **Profile logging level** - Built-in profiling support has arrived! Track performance with the new `profile_sync!` and `profile_async!` macros, plus a handy `#[profile]` attribute macro to instrument entire functions.
- **`profile_begin` macro** - Start profiling intervals manually when you need fine-grained control over what gets measured.
- **`ProfileInterval` type** - The building block of the profiling system, tracking timing data with automatic cleanup on drop.
- **Enhanced domain support** - Logging domains got a lot smarter, with better filtering and scoping so you can keep your logs organized even when things get chatty.

### Fixed

- **`debuginternal_sync!` crate detection** - Now correctly identifies whether it's being called from within the logwise crate itself. No more identity crises for internal debugging.
- **Improved line count accuracy** - Line numbers in logs are more reliable now, helping you pinpoint exactly where messages originate.

### Behind the Scenes

- Updated CI scripts to keep the build pipeline humming along
- Freshened up documentation to cover the new profiling features
- Dependency updates to stay current with the ecosystem
- Code formatting tweaks for consistency

## [0.4.0] - 2025-11-23

### Added

- **Heartbeat feature** - Keep tabs on long-running operations with the new heartbeat system. It'll let you know your process is still alive and kicking, even when things get quiet.
- **`log_enabled!` macro** - Now you can check if a log level is enabled before doing expensive formatting work. Your performance-conscious code will thank you.
- **`perfwarn_begin_if` macro** - Conditionally start performance warning intervals. Only track timing when you actually care about it.
- **SPDX license headers** - Every source file now properly identifies its licensing, because good housekeeping matters.
- **Comprehensive documentation** - Major docs overhaul across the codebase. We wrote the manual so you don't have to guess.

### Changed

- **Upgraded to Rust 2024 edition** - Living on the cutting edge with the latest Rust edition and rust-version 1.85.0.
- **`Context::new_task` API update** - Now takes `completion_level` and `should_log_completion` parameters, giving you finer control over task lifecycle logging. This is a breaking change, but the new expressiveness is worth the update.
- **Improved selective logging** - Smarter about when logging gets enabled, keeping things quieter when they should be.
- **Better log info propagation** - Context information flows through more reliably now.
- **Code organization improvements** - Split large modules into smaller, more focused files. The proc macro crate, context module, and macOS-specific code all got a spa day.

### Fixed

- **TLS crash** - Squashed a thread-local storage crash that could occur in certain conditions. Your threads can rest easy now.
- **CI builds** - Various fixes to keep the continuous integration humming along smoothly.
- **Clippy compliance** - Addressed linting issues because we believe in clean code.
- **`perfwarn_if` fixes** - Ironed out some wrinkles in the performance warning conditionals.

## [0.3.0] and earlier

For changes prior to version 0.4.0, please refer to the [commit history](https://github.com/drewcrawford/logwise/commits/main).
