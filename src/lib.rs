/*!
What's wrong with log?  Well –

"flat" log levels - without regard to current package, etc.
one logger - without regard to library authors may want their own analytics, etc
scales "wrong".  That is, to things like embedded rust (no_std), not to e.g. high-performance std profiling.
structured logging is "flat" and limited support for compiling them out

So, what do we want?

# dlog
* main package: 5-ish simple macros
* global, local, custom loggers

## kv:
provides an additional context object
reset, push, pop, etc

## perfwarn: convenience


*/

mod level;
mod logger;
mod privacy;
mod stderror_logger;
pub mod global_logger;
mod macros;

pub use level::Level;

#[doc(hidden)]
pub mod hidden {
    pub use crate::macros::{PrivateFormatter};
    pub use crate::global_logger::GLOBAL_LOGGER;
    pub use crate::logger::{Logger,LogRecord};

}