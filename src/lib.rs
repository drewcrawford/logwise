/*!
What's wrong with log?  Well –

"flat" log levels - without regard to current package, etc.
one logger - without regard to library authors may want their own analytics, etc
scales "wrong".  That is, to things like embedded rust (no_std), not to e.g. high-performance std profiling.
structured logging is "flat" and limited support for compiling them out

So, what do we want?

# dlog
* simple macros
* global, local loggers

* perfwarn: log a performance warning interval



*/


mod level;
mod logger;
mod privacy;
mod stderror_logger;
pub mod global_logger;
mod macros;
mod log_record;
pub mod interval;
pub mod context;
mod local_logger;

pub use level::Level;

pub use dlog_proc::{info_sync, perfwarn, debuginternal_async, debuginternal_sync, warn_sync};

#[doc(hidden)]
pub mod hidden {
    pub use crate::macros::{PrivateFormatter};
    pub use crate::global_logger::GLOBAL_LOGGER;
    pub use crate::logger::{Logger};
    pub use crate::log_record::LogRecord;
    pub use crate::macros::{debuginternal_pre,debuginternal_sync_post,debuginternal_async_post,info_sync_post,info_sync_pre,perfwarn_begin_post, perfwarn_begin_pre, warn_sync_pre, warn_sync_post};
}
extern crate self as dlog;
