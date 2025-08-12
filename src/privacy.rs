//SPDX-License-Identifier: MIT OR Apache-2.0
use crate::log_record::LogRecord;
use std::fmt::Debug;

pub trait Loggable {
    /**
    Logs the object to the provided builder.

    Use the representation that makes sense for public logging to a remote server.

    When implementing this, use of `#[inline]` is recommended.
    */
    fn log_redacting_private_info(&self, record: &mut LogRecord);

    /**
    Logs the object to the provided builder.

    Use the representation that makes sense for private logging.

    When implementing this, use of `#[inline]` is recommended.

    */
    fn log_all(&self, record: &mut LogRecord);
}

/**
u8 is probably not private.
*/
impl Loggable for u8 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        self.log_redacting_private_info(record);
    }
}

/**
u16 might be private.
*/
impl Loggable for u16 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<u16>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/**
u32 might be private.
*/
impl Loggable for u32 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<u32>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/**
u64 might be private.
*/
impl Loggable for u64 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<u64>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/**usize might be private.
*/
impl Loggable for usize {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<usize>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/**
u128 might be private.
*/
impl Loggable for u128 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<u128>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/**
i8 is probably not private.
*/
impl Loggable for i8 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        self.log_redacting_private_info(record);
    }
}

/**
i16 might be private.
*/
impl Loggable for i16 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<i16>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/**
i32 might be private.
*/
impl Loggable for i32 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<i32>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/**
i64 might be private.
*/
impl Loggable for i64 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<i64>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/**
i128 might be private.
*/
impl Loggable for i128 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<i128>")
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/**
f32 might be private.
*/
impl Loggable for f32 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<f32>");
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/**
f64 might be private.
*/
impl Loggable for f64 {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<f64>");
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/**
bool is probably not private.
*/
impl Loggable for bool {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/**
char is probably not private.
*/
impl Loggable for char {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{}", self));
    }
}

/**
String might be private.
*/
impl Loggable for String {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<String>");
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(self.clone());
    }
}

/**
&str might be private.
*/
impl Loggable for &str {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<&str>");
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(self.to_string());
    }
}

/**
slices depend on the underlying type.
*/
impl<T: Loggable> Loggable for &[T] {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<[");
        for item in self.iter() {
            item.log_redacting_private_info(record);
            record.log(", ");
        }
        record.log("]>");
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
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
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        match self {
            Some(t) => {
                record.log("Some(");
                t.log_redacting_private_info(record);
                record.log(")");
            }
            None => record.log("None"),
        }
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        match self {
            Some(t) => {
                record.log("Some(");
                t.log_all(record);
                record.log(")");
            }
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
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        record.log("<LogIt>");
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{:?}", self.0));
    }
}

/**
Escape hatch, we promise we aren't sending private data to servers.
*/
pub struct IPromiseItsNotPrivate<T>(pub T);

impl<T: Debug> Loggable for IPromiseItsNotPrivate<T> {
    #[inline]
    fn log_redacting_private_info(&self, record: &mut LogRecord) {
        Self::log_all(self, record);
    }
    #[inline]
    fn log_all(&self, record: &mut LogRecord) {
        record.log_owned(format!("{:?}", self.0));
    }
}
