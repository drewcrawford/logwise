# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`CLAUDE.md` is a symlink to `AGENTS.md`; edit either and both tools see it.

## Commands

CI runs the `scripts/` wrappers, not bare cargo. Use them — they set `RUSTFLAGS=-D warnings`, pass `--all` (so `logwise_runtime` and its temporary proc-macro crate are covered too), and supply the nightly/wasm flags. `scripts/check_all` also runs `scripts/facade_boundary`, which enforces the root facade's zero-dependency and no-alloc boundary.

`--all` means *the workspace*. The root manifest must continue to list `logwise_runtime` and `logwise_runtime/logwise_runtime_proc` explicitly: a path dependency is not automatically a workspace member, and dropping either member silently removes its clippy/tests/docs from the gate.

- `scripts/check_all` — facade boundary, fmt, check, clippy, tests, docs across both targets. The pre-PR gate.
- `scripts/fmt` — `cargo fmt --check`.
- `scripts/native/check` | `clippy` | `tests` | `docs` — native only.
- `scripts/wasm32/check` | `clippy` | `tests` | `docs` — wasm32 via `cargo +nightly`, the `wasm_lite run` browser runner, and the atomics/shared-memory flags in `.cargo/config.toml`. Needs `rustup +nightly target add wasm32-unknown-unknown`, `cargo install wasm_lite_cli`, and nightly `rust-src`.
- Any script accepts `--relaxed` to drop `-D warnings` while iterating.

Single test, iterating locally:

```
cargo test --test macro_hygiene_and_escapes          # one integration test file
cargo test test_escaped_braces                       # one test by name
cargo test --test perfwarn_if -- --nocapture
```

`tests/wasm_stderr_logger.rs` is `harness = false`; it is wired up explicitly in `Cargo.toml`.

### There are no dev-dependencies, and that is the point

Every dependency resolves from the registry; there is no `[patch.crates-io]` block and no path dependency outside this checkout. Keep it that way. The last dev-dependency was `test_executors`, and it cost more than it was worth: it depends on `logwise`, so the two could not be released independently, and its published expansion is `#[wasm_bindgen_test]`, which dragged the whole `wasm-bindgen`/`web-sys` tree — plus a *second, registry copy of `logwise`* — into the wasm32 test graph. Because `wasm_lite` exports `#[no_mangle]` symbols (`__wl_thread_entry`, `__wl_closure_call_0`, ...), a second copy of it in one graph is a wasm32 `duplicate symbol` link failure rather than a silent duplicate, so a stray dev-dependency pulling its own `wasm_lite` breaks the link outright.

The tests get everything they need from `wasm_lite`, which is already a wasm32 *normal* dependency. Off wasm32 the `cfg_attr` holding `#[wasm_lite_test]` is false and the cases are plain `#[test]`s, so the host build never needs the crate at all — which is why it is not also listed as a dev-dependency.

## Architecture

### The 0.7 package boundary

The root `logwise` package is the zero-dependency, `#![no_std]`, no-alloc-by-default facade. It must never regain clocks, TLS, threads, files, networking, wasm bindings, executors, owned records, concrete sinks, or implicit runtime behavior.

The facade's source is split by contract: `metadata.rs` owns the stable static schema axes; `value.rs` owns borrowed event/field values; `dispatch.rs` owns install-once dispatch and the generation-keyed call-site cache; `context.rs` owns only the opaque fixed-size token. A macro must call `Callsite::interest()` before evaluating any dynamic field, and all enabled observations use the one synchronous borrowed `EventRef` path. Targets without pointer-width atomics deliberately remain no-runtime no-ops.

`logwise_runtime/` contains the old implementation while it is ported to the new borrowed facade contract. `logwise_runtime/logwise_runtime_proc/` is temporary legacy machinery and must disappear when the declarative call-site macros land. `logwise_runtime_wasm/` owns the reserved host transport without depending on wasm_lite, and `logwise_integration_tests/` owns cross-package tests. Runtime and cross-package integration code may depend on the facade; the facade may depend on nothing. No runtime may depend on an executor.

### The legacy runtime contract

`logwise_runtime/logwise_runtime_proc/` generates the legacy macro bodies; `logwise_runtime/src/dispatch.rs` provides the functions they call, re-exported through `logwise_runtime::hidden` in `logwise_runtime/src/lib.rs`. Every legacy macro expands to the same three phases:

1. `logwise_runtime::hidden::<level>_pre(file!(), line!(), column!())` builds a `LogRecord` and stamps the context prelude onto it.
2. `PrivateFormatter` writes the message: `write_literal` for static text, `write_val` for each `{key}`.
3. `logwise_runtime::hidden::<level>_post(record)` fans the record out to the global loggers.

Adding or renaming a legacy macro means touching three places: the `logwise_runtime_proc` module that emits the source, `dispatch.rs` for the `_pre`/`_post` pair, and the `hidden` re-export list in `lib.rs`.

### The temporary runtime proc macros build source strings, not token streams

`logwise_runtime_proc` assembles Rust as a `String` and calls `.parse()`. Consequences that have already caused bugs:

- **No hygiene.** Every binding the expansion introduces is visible to call-site code spliced into it. Internal bindings are therefore `__logwise_`-prefixed (`__logwise_record`, `__logwise_formatter`, `__logwise_interval`, ...). Keep that convention for anything new.
- **`compile_error!` needs a terminating `;`.** It is spliced into statement position; without the semicolon it runs into the following statement and the user sees a syntax error pointing at generated code instead of the real message.
- **Never splice untrusted text into a generated string literal.** Key names come from the format string; quote them with `{:?}` rather than concatenating.
- `parser.rs::lformat_impl` walks the *source* form of the format literal, so escapes are still spelled out (`\n`, `\u{1F600}`). Literal runs are re-emitted verbatim into another literal, which round-trips — but any brace-scanning must skip escape sequences first.
- `parse_value` splits arguments on top-level commas, so it tracks angle-bracket depth to keep `foo::<A, B>()` intact. Commas inside `()`/`[]`/`{}` arrive as a single `Group` token and never reach it.

### `declare_logging_domain!()` is mandatory per crate

`log_enabled!` expands to reference `crate::__CALL_LOGWISE_DECLARE_LOGGING_DOMAIN`, the static that `declare_logging_domain!()` defines. Every crate — and every integration test file — that uses the logging macros must invoke it at its root, or expansion fails to resolve. The no-arg form compares `CARGO_CRATE_NAME` against `module_path!()` to decide whether `debuginternal` is on by default.

### Privacy is a dual-representation invariant

`PrivateFormatter::write_val` records each value twice, via `Loggable::log_all` and `Loggable::log_redacting_private_info`, into `LogRecord::parts` and `redacted_parts`. `LogRecord::clone_for_logger` then hands each logger the variant its `Logger::privacy()` allows. `LogPrivacy::Redacted` is the trait default, so a newly added logger cannot accidentally receive private data — only explicitly trusted local sinks (stderr, in-memory) override to `Private`. Preserve both representations in any code that constructs or forwards records.

Complex types have no blanket impl; callers must opt in through `privacy::LogIt` (redacted) or `privacy::IPromiseItsNotPrivate`. Note `Loggable` is implemented for owned values and `&str`/`&[T]`, not for `&T` generally, and `write_val` takes by value.

### Context and intervals

`context/` holds a thread-local stack of hierarchical `Context`s carrying a task, a nesting level, and a unique `ContextID`. Interval guards (`PerfwarnInterval`, `PerfwarnIntervalIf`, `ProfileInterval` in `interval.rs`) capture their `Context` at construction and log against *that* context on `Drop`, so a guard that crosses threads or outlives a context switch still reports where it began. `ApplyContext` carries a context across an async boundary. Anything that logs on drop should follow the same capture-at-construction pattern.

`global_logger.rs` guards the logger list with the crate's own `Spinlock`. Logger destructors are user code that may itself log, so retired loggers are dropped only after the configuration lock is released — do not shorten that dance.

### Time must go through `sys`

`src/sys.rs` re-exports `Duration`/`Instant` from `wasm_lite_std::time`. Use `crate::sys::Instant`, never `std::time::Instant`, or the wasm32 target breaks.

## Conventions

- Rust edition 2024, `rust-version` 1.85.1. Default rustfmt is the source of truth.
- Unit tests live beside the code; macro and end-to-end behavior goes in `tests/`. Name them `test_*`.
- **Every test runs on both targets.** A bare `#[test]` is a native-only test that compiles for wasm32 and then silently never runs — which is how the whole wasm32 suite went dark once already. Write the pair instead:

  ```rust
  #[cfg_attr(target_arch = "wasm32", wasm_lite::wasm_lite_test)]
  #[cfg_attr(not(target_arch = "wasm32"), test)]
  ```

  Use `wasm_lite_test(worker)` when the body blocks — `thread::sleep`, `join`, a contended lock — because those are `Atomics.wait`, which traps on the browser main thread. `std::thread::sleep` works on wasm32, but **`std::thread::spawn` does not**; go through `wasm_lite_std` for that, as `inmemory_logger.rs` and `heartbeat.rs` do. A test that genuinely cannot run in a browser (a subprocess, the filesystem) gets a file-level `#![cfg(not(target_arch = "wasm32"))]` **with a comment saying why** — the gap should be stated, not merely absent. `scripts/wasm32/tests` prints the case list; compare it against `scripts/native/tests` when adding a file.
- Tests touching global state call `Context::reset(...)`; tests that replace global loggers should restore them on drop (see `tests/unused_structured_fields.rs`) since test files share a process.
- Keep `perfwarn`/`heartbeat` thresholds generous — those tests are timing-sensitive.
- Changelog entries go under `Unreleased`, in commit order, in the existing conversational voice.
