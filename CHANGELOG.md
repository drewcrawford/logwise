# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
