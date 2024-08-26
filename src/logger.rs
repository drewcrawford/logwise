use std::fmt::{Debug, Display};

pub trait Logger: Debug  {
    type LogRecord: LogRecord;
    /**
    Creates a new log record.
    */
    fn new_log_record(&self) -> Self::LogRecord;

    /**
    Submits the log record for logging.
*/
    fn finish_log_record(&self, record: Self::LogRecord);

    /**
    Creates a new log record asynchronously.

    This allows loggers to reuse an async context that already exists.
    Loggers may choose to implement this as a simple wrapper around [new_log_record] if they wish.

    */
    async fn new_log_record_async(&self) -> Self::LogRecord;

    /**
    Submits the log record for logging asynchronously.

    This allows loggers to reuse an async context that already exists.
    Loggers may choose to implement this as a simple wrapper around [finish_log_record] if they wish.

    */
    async fn finish_log_record_async(&self, record: Self::LogRecord);

    /**
    The application may imminently exit.  Ensure all buffers are flushed and up to date.
    */
    fn prepare_to_die(&self);


}

/**
A log record.

We'd like to construct our API in a way that we don't need to allocate memory by concatenating strings, etc.

So instead our API assumes you progressively write a lot into somewhere.  However, due to the multithreaded
nature of logging, we need to be able to write to a buffer that is not shared between threads.

Instead, the design is as follows:

1.  Get a [LogRecord] from a [Logger].
2.  Progressively write to the [LogRecord].
3.  Finish the [LogRecord] and submit it to the [Logger].

*/
pub trait LogRecord: Debug + Clone + Display + From<Self::Logger> + Into<String> + Send + ToString {
    type Logger: Logger<LogRecord=Self>;
    /**
    Append the message to the record.

    This is called in the case that a message is not already owned.
    */
    fn log(&mut self, message: &str);

    /**
    Append the message to the record, taking ownership of the message.

    This is useful for messages that are already owned, such as those that are constructed in the process of logging.
    Logging implementations may choose to copy and drop the value if desired.
    */
    fn log_owned(&mut self, message: String);

    /**
    Append the message to the record asynchronously.

    This is called in the case that a message is not already owned.  Allows the log record to reuse an async context
    that already exists, if needed.

    Implementations may choose to implement this as a simple wrapper around [log] if they wish.
    */
    async fn log_async(&mut self, message: &str);

    /**
    Append the message to the record, taking ownership of the message, asynchronously.

    This is useful for messages that are already owned, such as those that are constructed in the process of logging.
    Logging implementations may choose to copy and drop the value if desired.

    Implementations may choose to implement this as a simple wrapper around [log_owned] if they wish.
    */

    async fn log_owned_async(&mut self, message: String);

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

# LogRecord

I think Clone probably makes sense although I'm not sure how much use it would get necessarily.
PartialEq and Eq are possible but it's a little unclear if we mean data equality or some kind of provenance-based thing.  Let's avoid that and not implement it.
Ord makes no sense
Hash makes a little more sense but I'm still not sure if we mean data equality or provenance-based equality.
Default is not sensible as we need a logger to construct it.
Display is probably sensible; we can display the log record.
Can't implement From/Into due to the orphan rule but we can require implementors do so.
AsRef/AsMut - not necessarily a great implementation.
Defef - must deref to a pointer type, which we don't necessarily have
Send - yes, Sync - no
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
    type LogRecord = AnyLogRecord<L::LogRecord>;

    fn new_log_record(&self) -> Self::LogRecord {
        AnyLogRecord(self.0.new_log_record())
    }

    fn finish_log_record(&self, record: Self::LogRecord) {
        self.0.finish_log_record(record.0)
    }

    async fn new_log_record_async(&self) -> Self::LogRecord {
        AnyLogRecord(self.0.new_log_record_async().await)
    }

    async fn finish_log_record_async(&self, record: Self::LogRecord) {
        self.0.finish_log_record_async(record.0).await
    }

    fn prepare_to_die(&self) {
        self.0.prepare_to_die()
    }
}

#[derive(Debug)]
pub struct AnyLogRecord<R: LogRecord>(R);

impl<R: LogRecord> LogRecord for AnyLogRecord<R> {
    type Logger = AnyLogger<R::Logger>;

    fn log(&mut self, message: &str) {
        self.0.log(message)
    }

    fn log_owned(&mut self, message: String) {
        self.0.log_owned(message)
    }

    async fn log_async(&mut self, message: &str) {
        self.0.log_async(message).await
    }

    async fn log_owned_async(&mut self, message: String) {
        self.0.log_owned_async(message).await
    }
}

impl<R: LogRecord> Clone for AnyLogRecord<R> {
    fn clone(&self) -> Self {
        AnyLogRecord(self.0.clone())
    }
}

impl<R: LogRecord> Display for AnyLogRecord<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<R: LogRecord> From<AnyLogger<R::Logger>> for AnyLogRecord<R> {
    fn from(value: AnyLogger<R::Logger>) -> Self {
        AnyLogRecord(value.0.new_log_record())
    }
}

impl<R: LogRecord> Into<String> for AnyLogRecord<R> {
    fn into(self) -> String {
        self.0.into()
    }
}