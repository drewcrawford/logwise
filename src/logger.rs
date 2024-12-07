//SPDX-License-Identifier: MIT OR Apache-2.0
use std::fmt::{Debug};
use std::future::Future;
use crate::hidden::LogRecord;

pub trait Logger: Debug  {

    /**
    Submits the log record for logging.
*/
    fn finish_log_record(&self, record: LogRecord);



    /**
    Submits the log record for logging asynchronously.

    This allows loggers to reuse an async context that already exists.
    Loggers may choose to implement this as a simple wrapper around [Self::finish_log_record] if they wish.

    */
    fn finish_log_record_async<'s>(&'s self, record: LogRecord) -> impl Future<Output=()> + 's;

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

/**
A logger that hides, without erasing, the underlying type of logger.
*/
#[derive(Debug)]
pub struct AnyLogger<L: Logger>(L);

impl<L: Logger> AnyLogger<L> {
    pub const fn new(logger: L) -> AnyLogger<L> {
        AnyLogger(logger)
    }
}

impl<L: Logger> Logger for AnyLogger<L> {


    fn finish_log_record(&self, record: LogRecord) {
        self.0.finish_log_record(record)
    }


    async fn finish_log_record_async(&self, record: LogRecord) {
        self.0.finish_log_record_async(record).await
    }

    fn prepare_to_die(&self) {
        self.0.prepare_to_die()
    }
}

