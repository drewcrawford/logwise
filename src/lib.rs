// SPDX-License-Identifier: MIT OR Apache-2.0
/*!
# logwise

An opinionated logging library for Rust with structured logging, privacy-aware data handling,
and hierarchical task-based context management.

# Development Status

logwise is experimental and the API may change.

# The Problem

Traditional logging crates like [`log`](https://crates.io/crates/log) offer generic log levels
(`error`, `warn`, `info`, `debug`, `trace`) that are often too vague for real-world use cases.
This leads to several problems:

* **Ambiguous levels**: Is `debug` for print-style debugging in your library, or for users
  debugging their own code?
* **Build control**: How do you compile out expensive logs by default but enable them when
  debugging user-reported issues?
* **Missing use cases**: What level is appropriate for "this is slow and should be optimized"?

logwise solves these problems with opinionated, use-case-specific log levels.

# Core Philosophy

Think of log levels like module visibility. You have `pub`, `pub(crate)`, `pub(super)`, and
private visibility for different use cases. logwise provides similar granular control for logging.

# Log Levels

logwise provides specific log levels for defined use cases:

| Level | Use Case | Build Type | Thread Control |
|-------|----------|------------|----------------|
| `trace` | Detailed debugging | debug only | Must enable per-thread via [`Context::begin_trace()`](context::Context::begin_trace) |
| `debuginternal` | Print-style debugging | debug only | On by default in current crate, per-thread in downstream |
| `info` | Supporting downstream crates | debug only | On by default |
| `perfwarn` | Performance problems with analysis | all builds | Always on |
| `warning` | Suspicious conditions | all builds | Always on |
| `error` | Logging errors in Results | all builds | Always on |
| `panic` | Programmer errors | all builds | Always on |

# Quick Start

## Basic Logging

```rust
// Simple structured logging
logwise::info_sync!("User logged in", user_id=42);

// With multiple parameters
logwise::warn_sync!("Request failed",
    status=404,
    path="/api/users"
);
```

```rust
// Async logging for better performance
async fn handle_request() {
    logwise::info_async!("Processing request",
        method="GET",
        endpoint="/health"
    );
}
```

## Privacy-Aware Logging

logwise's privacy system ensures sensitive data is handled appropriately:

```rust
use logwise::privacy::{LogIt, IPromiseItsNotPrivate};

#[derive(Debug)]
struct User {
    id: u64,
    name: String,
    email: String,
}

// Complex types require explicit privacy handling
let user = User {
    id: 123,
    name: "Alice".into(),
    email: "alice@example.com".into()
};

// Use LogIt wrapper for complex types
logwise::info_sync!("User created", user=LogIt(&user));

// Mark explicitly non-private data when it's safe
let public_id = "PUBLIC-123";
logwise::info_sync!("Processing {id}",
    id=IPromiseItsNotPrivate(public_id)
);
```

## Context and Task Management

Track hierarchical tasks with automatic performance monitoring:

```rust
# fn main() {
use logwise::context::Context;

// Create a new task
let ctx = Context::new_task(
    Some(Context::current()),
    "data_processing".to_string()
);
ctx.clone().set_current();

// Enable detailed tracing for debugging
Context::begin_trace();

// Nested tasks automatically track hierarchy
let child_ctx = Context::new_task(
    Some(Context::current()),
    "parse_csv".to_string()
);
child_ctx.clone().set_current();

// Task completion is logged automatically when dropped
# }
```

## Performance Tracking

Use the `perfwarn!` macro to track and log slow operations:

```rust
# fn perform_database_query() {}
// Tracks execution time automatically
logwise::perfwarn!("database_query", {
    // Your expensive operation here
    perform_database_query()
});
// Logs warning if operation exceeds threshold
```

# Architecture Overview

## Core Components

* **[`Logger`]**: The trait defining logging backends
* **[`LogRecord`]**: Structured log entry with metadata
* **[`Level`]**: Enumeration of available log levels
* **[`context`] module**: Thread-local hierarchical task management
* **[`privacy`] module**: Privacy-aware data handling system
* **[`global_logger`] module**: Global logger registration and management

## Logging Flow

1. Macros generate a [`LogRecord`] with source location metadata
2. The [`privacy::Loggable`] trait determines how values are logged
3. Records are dispatched to registered [`Logger`] implementations
4. Loggers can process records synchronously or asynchronously

# Examples

## Complete Application Example

```rust
use logwise::{context::Context, privacy::LogIt};

#[derive(Debug)]
struct Config {
    database_url: String,
    port: u16,
}

fn main() {
    // Initialize root context
    Context::reset("application".to_string());

    // Log application startup
    logwise::info_sync!("Starting application", version="1.0.0");

    // Create task for initialization
    let init_ctx = Context::new_task(
        Some(Context::current()),
        "initialization".to_string()
    );
    init_ctx.clone().set_current();

    // Load configuration
    let config = Config {
        database_url: "postgres://localhost/myapp".into(),
        port: 8080,
    };

    // Use LogIt for complex types
    logwise::info_sync!("Configuration loaded",
        config=LogIt(&config)
    );

    // Track performance-critical operations
    logwise::perfwarn!("database_connection", {
        // connect_to_database(&config.database_url)
    });

    logwise::info_sync!("Application ready", port=config.port);
}
```

## Custom Logger Implementation

```rust
use logwise::{Logger, LogRecord};
use std::sync::Arc;
use std::fmt::Debug;
use std::pin::Pin;
use std::future::Future;

#[derive(Debug)]
struct CustomLogger;

impl Logger for CustomLogger {
    fn finish_log_record(&self, record: LogRecord) {
        // Custom logging logic
        eprintln!("[CUSTOM] {}", record);
    }

    fn finish_log_record_async<'s>(&'s self, record: LogRecord) -> Pin<Box<dyn Future<Output = ()> + Send + 's>> {
        Box::pin(async move {
            // Async logging logic
            eprintln!("[CUSTOM ASYNC] {}", record);
        })
    }

    fn prepare_to_die(&self) {
        // Cleanup logic
    }
}

// Register the logger
logwise::add_global_logger(Arc::new(CustomLogger));
```

# Thread Safety and Async Support

logwise is designed for concurrent and async environments:

* All [`Logger`] implementations must be `Send + Sync`
* Context is thread-local with parent inheritance
* [`context::ApplyContext`] preserves context across async boundaries
* Both sync and async logging variants are available

# Performance Considerations

* Use async variants (`*_async!`) in async contexts for better performance
* The `perfwarn!` macro includes automatic interval tracking
* Debug-only levels are compiled out in release builds
* Thread-local caching reduces synchronization overhead

# WASM Support

logwise includes WebAssembly support with browser-specific features:

* Uses `web-time` for time operations
* Console output via `web-sys`
* Batched logging with [`InMemoryLogger::periodic_drain_to_console`](InMemoryLogger::periodic_drain_to_console)

*/

pub mod context;
pub mod global_logger;
mod inmemory_logger;
pub mod interval;
mod level;
mod local_logger;
mod log_record;
mod logger;
mod macros;
pub mod privacy;
mod spinlock;
mod stderror_logger;
mod sys;

declare_logging_domain!();

// Re-export core types and functions for public API
pub use global_logger::{add_global_logger, global_loggers, set_global_loggers};
pub use inmemory_logger::InMemoryLogger;
pub use level::Level;
pub use log_record::LogRecord;
pub use logger::Logger;

// Re-export logging macros from the procedural macro crate.
// See individual macro documentation for usage details.
pub use logwise_proc::{
    debuginternal_async, debuginternal_sync, error_async, error_sync, info_async, info_sync,
    perfwarn, perfwarn_begin, trace_async, trace_sync, warn_sync,
};

pub use macros::LoggingDomain;

#[doc(hidden)]
pub mod hidden {
    pub use crate::global_logger::global_loggers;
    pub use crate::macros::PrivateFormatter;
    pub use crate::macros::{
        debuginternal_async_post, debuginternal_pre, debuginternal_sync_post, error_async_post,
        error_sync_post, error_sync_pre, info_async_post, info_sync_post, info_sync_pre,
        perfwarn_begin_post, perfwarn_begin_pre, trace_async_post, trace_sync_post, trace_sync_pre,
        warn_sync_post, warn_sync_pre,
    };
}
extern crate self as logwise;

pub use sys::Duration;
