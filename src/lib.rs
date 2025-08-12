//SPDX-License-Identifier: MIT OR Apache-2.0
/*!
# logwise

logwise is an opinionated logging library for Rust.

# Development status

logwise is experimental and the API may change.

# The problem

Typical logging crates, such as [log](https://crates.io/crates/log), offer small set of generic (that is, vague) log levels (`error`, `warn`, `info`, `debug`, `trace`). But these are so vague and the implementations of loggers so varied that I am never really sure which one to use.

Here are some problems:

* Suppose I am doing print-style debugging in my library.  Is `debug` the right level to use?  Or is `debug` where I put messages for *users* of my library to debug *their own* code?
* How can I compile-out expensive logs by default but collect them from users when they report a bug?
* What's the appropriate log level for "This is slow and should be optimized"?

These problems cannot be solved within the ecosystem-wide common-denominator API, so here we are.

# An analogy

There is a simple analogy to *module visibility*.  There are usecases to make a module private.  There are usecases to make it public.  To make it `pub(crate)`.  To make it `pub(super)`, except if it's a release build, and so on.  logwise provides tools like that, but for logging.

# The facade

logwise provides an opinionated set of log levels for defined set of usecases.


| Name          | Usecase                                 | Build type required | Conditions                                                                         |
|---------------|-----------------------------------------|---------------------|------------------------------------------------------------------------------------|
| trace         | Detailed debugging                      | debug builds only   | Must turn on per-thread                                                            |
| debuginternal | print-style debugging                   | debug builds only   | On by default in the current crate.  Must turn on per-thread in downstream crates. |
| info          | Supporting downstream crates            | debug builds only   | On by default                                                                      |
| perfwarn      | Log performance problems, with analysis | all                 | all                                                                                |
| warning       | Suspicious condition                    | all                 | all                                                                                |
| error         | logging the error in a `Result`         | all                 | all                                                                                |
| panic         | logging a programmer error              | all                 | all                                                                                |

(More levels may be added).

# The implementation

logwise currently logs all messages to stderr.  In the future, other logging backends may be added.

# The API

For example,

```rust
# let val = false;
  logwise::info_sync!("Hello {world}!",world=val);
```

See the docs for more information.  Each log level has a synchronous and asynchronous version.  The synchronous version
can be used from any context.  The asynchronous version allows the logging to be deferred to your async executor for
better performance in some cases.

The API supports a simple key-value syntax for structured logging.  The right-hand side of the expression
is only evaluated if the log message is actually printed, and is compiled out when necessary.

# Privacy

Consider the following log message:

```text
Completed job 23 named 'Gift for Alice' in 3.4 seconds.
```

Another gripe I have about the `log` crate's API is there is no way to represent which parts of the log may contain
sensitive user data (in this case, the name of the job).  It may be useful to collect this log, but it may be
undesirable to include the sensitive variable in the log message.

logwise's design is designed each variable in the log message conforms to the `Loggable` trait, which defines a public
and private representation of the variable.  This allows different versions of the log to be generated
from the same log message, depending on the desired privacy level.

# Multithreading

logwise has out-of-the-box support for scoped logging in a stack-based, thread-local model.
If you are e.g. spawning a child thread, writing an async executor or similar,
consider using the `logwise::context` APIs to propagate the logging context to child threads.








*/


mod level;
mod logger;
pub mod privacy;
mod stderror_logger;
mod inmemory_logger;
pub mod global_logger;
mod macros;
mod log_record;
pub mod interval;
pub mod context;
mod local_logger;
mod sys;
mod spinlock;

declare_logging_domain!();

pub use level::Level;
pub use logger::Logger;
pub use log_record::LogRecord;
pub use inmemory_logger::InMemoryLogger;
pub use global_logger::{add_global_logger, set_global_loggers, global_loggers};

pub use logwise_proc::{info_sync, perfwarn, debuginternal_async, debuginternal_sync, warn_sync,perfwarn_begin,info_async, trace_sync, trace_async,error_sync, error_async};

pub use macros::LoggingDomain;



#[doc(hidden)]
pub mod hidden {
    pub use crate::macros::{PrivateFormatter};
    pub use crate::global_logger::global_loggers;
    pub use crate::macros::{debuginternal_pre,debuginternal_sync_post,debuginternal_async_post,
                            info_sync_post,info_sync_pre,info_async_post,
                            perfwarn_begin_post, perfwarn_begin_pre,
                            warn_sync_pre, warn_sync_post,
                            trace_sync_pre, trace_sync_post, trace_async_post,
                            error_sync_pre, error_sync_post, error_async_post};
}
extern crate self as logwise;

pub use sys::Duration;