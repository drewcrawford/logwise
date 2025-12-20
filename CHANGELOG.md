# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
