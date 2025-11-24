# Repository Guidelines

## Project Structure & Module Organization
- `src/`: Core crate with logging macros, context management, privacy helpers, and logger backends. Keep new modules scoped and wire them through `lib.rs`.
- `logwise_proc/`: Procedural macro crate backing the public macros. Mirror any macro-facing changes here.
- `tests/`: Integration tests for perf warnings, heartbeats, and macros; prefer adding new end-to-end coverage here.
- `art/` holds assets (logo). `compare_api.sh` / `compare_docs.sh` help diff generated outputs if you use them locally.

## Build, Test, and Development Commands
- `cargo fmt --check` — rustfmt gate; required before commits.
- `cargo check` — fast sanity check with warnings denied in CI (`-D warnings` via cargo config); fix all warnings locally.
- `cargo clippy --no-deps -- -D warnings` — lints must be clean.
- `cargo test` — runs unit + integration tests in `tests/`.
- `cargo +nightly test --target wasm32-unknown-unknown` — WASM suite via `wasm-bindgen-test-runner` (configured in `.cargo/config.toml`); install the target and `wasm-bindgen-cli` first.
- `cargo doc` — ensure docs build; CI treats doc warnings as errors.

## Coding Style & Naming Conventions
- Rust edition 2024; rust-version 1.85.0. Default rustfmt (4-space indent, trailing commas) is the source of truth.
- Keep modules and files `snake_case`; types `PascalCase`; methods/functions `snake_case`; constants `SCREAMING_SNAKE_CASE`.
- Prefer explicit privacy handling (`privacy::Loggable`, `LogIt`, `IPromiseItsNotPrivate`) when logging complex data.
- Favor async variants (`*_async!`) in async contexts; keep sync/async naming consistent.

## Testing Guidelines
- Add unit tests near the code they cover; use integration tests in `tests/` for macro and behavior coverage.
- Name tests `test_*` with descriptive scenarios; isolate global state by calling `Context::reset(...)`.
- For timing-sensitive paths (`perfwarn`, `heartbeat`), keep thresholds generous to avoid flaky runs.
- Run the WASM target tests when touching cross-platform code; `.cargo/config.toml` sets the runner and atomic flags.

## Commit & Pull Request Guidelines
- PRs should describe the behavior change, note any API impacts, and link related issues. Include screenshots only if UI/visual output changes.
- Before opening a PR: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and the WASM test command when relevant.
