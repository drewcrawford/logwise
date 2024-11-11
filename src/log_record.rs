//SPDX-License-Identifier: MIT OR Apache-2.0
use std::fmt::{Debug, Display};
use std::sync::OnceLock;
use crate::Level;

static INITIAL_TIMESTAMP: OnceLock<crate::sys::Instant> = OnceLock::new();

fn initial_timestamp() -> crate::sys::Instant {
    *INITIAL_TIMESTAMP.get_or_init(|| crate::sys::Instant::now())
}

/**
A log record.

We'd like to construct our API in a way that we don't need to allocate memory by concatenating strings, etc.

So instead our API assumes you progressively write a lot into somewhere.  However, due to the multithreaded
nature of logging, we need to be able to write to a buffer that is not shared between threads.

Instead, the design is as follows:

1.  Create a new [LogRecord].
2.  Progressively write to the [LogRecord].
3.  Finish the [LogRecord] and submit it to the [logwise::logger::Logger].

*/
#[derive(Debug,Clone,PartialEq, Hash)]
pub struct LogRecord {
    pub(crate) parts: Vec<String>,
    level: Level,
}
impl LogRecord {
    /**
    Append the message to the record.

    This is called in the case that a message is not already owned.
    */
    pub fn log(&mut self, message: &str) {
        self.parts.push(message.to_string());
    }

    /**
    Append the message to the record, taking ownership of the message.

    This is useful for messages that are already owned, such as those that are constructed in the process of logging.
    Logging implementations may choose to copy and drop the value if desired.
    */
    pub fn log_owned(&mut self, message: String) {
        self.parts.push(message);
    }

    pub fn new(level: Level) -> Self {
        Self {
            parts: Vec::new(),
            level,
        }
    }

    /**
    Log the current time to the record, followed by a space.
    */
    pub fn log_timestamp(&mut self) -> crate::sys::Instant {
        let time = crate::sys::Instant::now();
        let duration = time.duration_since(initial_timestamp());
        self.log_owned(format!("[{:?}] ", duration));
        time
    }

    pub fn log_time_since(&mut self, start: crate::sys::Instant) {
        let duration = start.duration_since(initial_timestamp());
        self.log_owned(format!("[{:?}] ", duration));
    }
    pub fn level(&self) -> Level {
        self.level
    }
}

impl Default for LogRecord {
    fn default() -> Self {
        Self::new(Level::Info)
    }
}

impl Display for LogRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for part in &self.parts {
            write!(f, "{}", part)?;
        }
        Ok(())
    }
}
/*
Boilerplate notes
# LogRecord

I think Clone probably makes sense although I'm not sure how much use it would get necessarily.
PartialEq and Eq make sense
Ord makes no sense
Hash
Default
Display is probably sensible; we can display the log record.
From/Into - not very sensible
AsRef/AsMut - not necessarily a great implementation.
Defef - must deref to a pointer type, which we don't necessarily have
Send - yes, Sync - no
 */