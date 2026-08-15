# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`CLAUDE.md` is a symlink to `AGENTS.md`; edit either and both tools see it.

## Commands

CI runs the `scripts/` wrappers, not bare cargo. Use them — they set `RUSTFLAGS=-D warnings`, pass `--all` (so `logwise_proc` is covered too), and supply the nightly/wasm flags.

- `scripts/check_all` — fmt, check, clippy, tests, docs across both targets. The pre-PR gate.
- `scripts/fmt` — `cargo fmt --check`.
- `scripts/native/check` | `clippy` | `tests` | `docs` — native only.
- `scripts/wasm32/check` | `clippy` | `tests` | `docs` — wasm32 via `cargo +nightly`, `wasm-bindgen-test-runner`, and the atomics/shared-memory flags in `.cargo/config.toml`. Needs `rustup +nightly target add wasm32-unknown-unknown`, `cargo +nightly install wasm-bindgen-cli`, and nightly `rust-src`.
- Any script accepts `--relaxed` to drop `-D warnings` while iterating.

Single test, iterating locally:

```
cargo test --test macro_hygiene_and_escapes          # one integration test file
cargo test test_escaped_braces                       # one test by name
cargo test --test perfwarn_if -- --nocapture
```

`tests/wasm_stderr_logger.rs` is `harness = false`; it is wired up explicitly in `Cargo.toml`.

### Local path dependencies

The root `Cargo.toml` depends on `../wasm_lite/crates/wasm_lite_std` by path and carries a `[patch.crates-io]` block pointing `some_executor`, `test_executors`, `wasm_lite`, `wasm_lite_std`, and `logwise` itself at sibling checkouts. If any sibling is missing, **every** cargo command fails during resolution with `failed to load source for dependency`, before compiling anything. Clone the siblings, or temporarily swap the missing one for its crates.io release — and restore `Cargo.toml`/`Cargo.lock` before committing.

Each entry earns its place; check before adding or removing one. `some_executor`, `test_executors`, and `logwise` are patched because their published releases still carry `wasm-bindgen`/`web-sys` on a wasm32 *normal* dependency, and the `Instant` flag-day is all-or-nothing. `wasm_lite` and `wasm_lite_std` are patched for a different reason: we take them by path while `some_executor` takes them from the registry, and because `wasm_lite` exports `#[no_mangle]` symbols (`__wl_thread_entry`, `__wl_closure_call_0`, ...), two copies in one graph is a wasm32 `duplicate symbol` link failure rather than a silent duplicate. `continue` needs no patch as of its 0.1.3 release. To check whether a patch is still needed, read the published entry's *normal* deps rather than guessing — `cargo info` reports the patched copy, so it cannot tell you.

## Architecture

### Two crates, one contract

`logwise_proc/` generates the macro bodies; `src/dispatch.rs` provides the functions they call, re-exported through `logwise::hidden` in `src/lib.rs`. Every macro expands to the same three phases:

1. `logwise::hidden::<level>_pre(file!(), line!(), column!())` builds a `LogRecord` and stamps the context prelude onto it.
2. `PrivateFormatter` writes the message: `write_literal` for static text, `write_val` for each `{key}`.
3. `logwise::hidden::<level>_post(record)` fans the record out to the global loggers.

Adding or renaming a macro means touching three places: the `logwise_proc` module that emits the source, `dispatch.rs` for the `_pre`/`_post` pair, and the `hidden` re-export list in `lib.rs`.

### The proc macros build source strings, not token streams

`logwise_proc` assembles Rust as a `String` and calls `.parse()`. Consequences that have already caused bugs:

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
- Tests touching global state call `Context::reset(...)`; tests that replace global loggers should restore them on drop (see `tests/unused_structured_fields.rs`) since test files share a process.
- Keep `perfwarn`/`heartbeat` thresholds generous — those tests are timing-sensitive.
- Changelog entries go under `Unreleased`, in commit order, in the existing conversational voice.
