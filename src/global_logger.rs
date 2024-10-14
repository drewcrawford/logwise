//SPDX-License-Identifier: MIT OR Apache-2.0
use crate::logger::AnyLogger;
use crate::stderror_logger::StdErrorLogger;

/**
The global logger.

For perf reasons, we want the type to be known statically.  However, we also want to be able to swap
the type for a different one at compile-time.  Therefore, the API contract here provides an `AnyLogger`.

 */
pub static GLOBAL_LOGGER: AnyLogger<StdErrorLogger> = AnyLogger::new(StdErrorLogger::new());