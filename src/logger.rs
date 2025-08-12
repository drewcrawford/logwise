//SPDX-License-Identifier: MIT OR Apache-2.0
use crate::log_record::LogRecord;
use std::fmt::Debug;

pub trait Logger: Debug + Send + Sync {
    /**
        Submits the log record for logging.
    */
    fn finish_log_record(&self, record: LogRecord);

    /**
    Submits the log record for logging asynchronously.

    This allows loggers to reuse an async context that already exists.
    Loggers may choose to implement this as a simple wrapper around [Self::finish_log_record] if they wish.

    */
    fn finish_log_record_async<'s>(
        &'s self,
        record: LogRecord,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 's>>;

    /**
    The application may imminently exit.  Ensure all buffers are flushed and up to date.
    */
    fn prepare_to_die(&self);
}

/*
Boilerplate notes.

# Logger

I don't think Clone on Logger makes sense, so copy's out.
PartialEq and Eq are possible but it's a little unclear if we mean data equality or some kind of provenance-based thing.  Let's avoid that and not implement it.
Ord makes no sense
Hash makes a little more sense but I'm still not sure if we mean data equality or provenance-based equality.
Default is not necessarily sensible since who knows how the logger is constructed (does it need a filename to log to, etc.)
Display is not very sensible.
From/Into, no
AsRef,AsMut,Deref,DerefMut, no
Send/Sync makes sense for typical loggers but I could imagine corner cases where they aren't.


*/
