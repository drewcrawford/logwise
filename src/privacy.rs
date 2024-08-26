use std::fmt::Debug;
use crate::logger::LogRecord;

pub trait Loggable {
    /**
    Logs the object to the provided builder.

    Use the representation that makes sense for public logging to a remote server.

    When implementing this, use of `#[inline]` is recommended.
    */
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record);

    /**
    Logs the object to the provided builder.

    Use the representation that makes sense for private logging.

    When implementing this, use of `#[inline]` is recommended.

    */
    fn log_all<Record: LogRecord>(&self, record: &mut Record);
}

/**
u8 is probably not private.
*/
impl Loggable for u8 {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        self.log_redacting_private_info(record);
    }
}

/**
u16 might be private.
*/

impl Loggable for u16 {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log("<u16>")
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
}

/**
u32 might be private.
*/

impl Loggable for u32 {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log("<u32>")
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
}

/**
u64 might be private.
*/

impl Loggable for u64 {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log("<u64>")
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
}

/**
u128 might be private.
*/

impl Loggable for u128 {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log("<u128>")
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
}

/**
i8 is probably not private.
*/

impl Loggable for i8 {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        self.log_redacting_private_info(record);
    }
}

/**
i16 might be private.
*/

impl Loggable for i16 {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log("<i16>")
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
}

/**
i32 might be private.
*/

impl Loggable for i32 {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log("<i32>")
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
}

/**
i64 might be private.
*/

impl Loggable for i64 {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log("<i64>")
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
}

/**
i128 might be private.
*/

impl Loggable for i128 {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log("<i128>")
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
}

/**
f32 might be private.
*/

impl Loggable for f32 {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log("<f32>");
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
}

/**
f64 might be private.
*/

impl Loggable for f64 {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log("<f64>");
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
}

/**
bool is probably not private.
*/

impl Loggable for bool {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
}

/**
char is probably not private.
*/

impl Loggable for char {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{}", self));
    }
}

/**
String might be private.
*/

impl Loggable for String {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log("<String>");
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(self.clone());
    }
}

/**
&str might be private.
*/

impl Loggable for &str {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log("<&str>");
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(self.to_string());
    }
}

/**
slices depend on the underlying type.
*/

impl<T: Loggable> Loggable for &[T] {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log("<[");
        for item in self.iter() {
            item.log_redacting_private_info(record);
            record.log(", ");
        }
        record.log("]>");
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log("<[");
        for item in self.iter() {
            item.log_all(record);
            record.log(", ");
        }
        record.log("]>");
    }
}

/**
Option depends on the underlying type.
*/

impl<T: Loggable> Loggable for Option<T> {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        match self {
            Some(t) => {
                record.log("Some(");
                t.log_redacting_private_info(record);
                record.log(")");
            },
            None => record.log("None"),
        }
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        match self {
            Some(t) => {
                record.log("Some(");
                t.log_all(record);
                record.log(")");
            },
            None => record.log("None"),
        }
    }
}

/**
Escape hatch, we won't send any data to servers.
*/

pub struct LogIt<T>(pub T);

impl<T: Debug> Loggable for LogIt<T> {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        record.log("<LogIt>");
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        record.log_owned(format!("{:?}", self.0));
    }
}


/**
Escape hatch, we promise we aren't sending private data to servers.
*/
pub struct IPromiseItsNotPrivate<T>(pub T);

impl<T: Loggable> Loggable for IPromiseItsNotPrivate<T> {
    #[inline]
    fn log_redacting_private_info<Record: LogRecord>(&self, record: &mut Record) {
        self.0.log_all(record);
    }
    #[inline]
    fn log_all<Record: LogRecord>(&self, record: &mut Record) {
        self.0.log_all(record);
    }
}