# dlog

![logo](art/logo.png)

dlog is an opinionated logging library for Rust.

# Development status

dlog is experimental and the API may change.

# The problem

Typical logging crates, such as [log](https://crates.io/crates/log), offer small set of generic (that is, vague) log levels (`error`, `warn`, `info`, `debug`, `trace`). But these are so vague and the implementations of loggers so varied that I am never really sure which one to use.

Here are some problems:

* Suppose I am doing print-style debugging in my library.  Is `debug` the right level to use?  Or is `debug` where I put messages for *users* of my library to debug *their own* code?   
* How can I compile-out expensive logs by default but collect them from users when they report a bug?
* What's the appropriate log level for "This is slow and should be optimized"?

These problems cannot be solved within the ecosystem-wide common-denominator API, so here we are.

# An analogy

There is a simple analogy to *module visibility*.  There are usecases to make a module private.  There are usecases to make it public.  To make it `pub(crate)`.  To make it `pub(super)`, except if it's a release build, and so on.  dlog provides tools like that, but for logging.

# The facade

dlog provides an opinionated set of log levels for defined set of usecases.


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

dlog currently logs all messages to stderr.  In the future, other logging backends may be added.

# The API

For example,

```rust
        dlog::debuginternal_sync!("Hello {world}!",world=val);
```

See the docs for more information.  Each log level has a synchronous and asynchronous version.  The synchronous version
can be used from any context.  The asynchronous version allows the logging to be deferred to your async executor for
better performance in some cases.

The API supports a simple key-value syntax for structured logging.  The right-hand side of the expression
is only evaluated if the log message is actually printed, and is compiled out when necessary.

# Privacy

Consider the following log message:

```
Completed job 23 named 'Gift for Alice' in 3.4 seconds.
```

Another gripe I have about the `log` crate's API is there is no way to represent which parts of the log may contain
sensitive user data (in this case, the name of the job).  It may be useful to collect this log, but it may be 
undesirable to include the sensitive variable in the log message.

dlog's design is designed each variable in the log message conforms to the `Loggable` trait, which defines a public
and private representation of the variable.  This allows different versions of the log to be generated
from the same log message, depending on the desired privacy level.

# Multithreading

dlog has out-of-the-box support for scoped logging in a stack-based, thread-local model.
If you are e.g. spawning a child thread, writing an async executor or similar,
consider using the `dlog::context` APIs to propagate the logging context to child threads.




