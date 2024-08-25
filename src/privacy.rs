use std::fmt::Debug;

/// an in-progress log.
pub trait LogBuilder {
    fn write(&mut self, message: &str);
}
pub trait Loggable {
    /**
    Logs the object to the provided builder.

    Use the representation that makes sense for public logging

    When implementing this, use of `#[inline]` is recommended.
    */
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder);

    /**
    Logs the object to the provided builder.

    Use the representation that makes sense for private logging.

    When implementing this, use of `#[inline]` is recommended.

    */
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder);
}

/**
u8 is probably not private.
*/
impl Loggable for u8 {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{}", self));
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        self.log_redacting_private_info(builder);
    }
}

/**
u16 might be private.
*/

impl Loggable for u16 {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write("<u16>>");
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{}", self));
    }
}

/**
u32 might be private.
*/

impl Loggable for u32 {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write("<u32>");
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{}", self));
    }
}

/**
u64 might be private.
*/

impl Loggable for u64 {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write("<u64>");
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{}", self));
    }
}

/**
u128 might be private.
*/

impl Loggable for u128 {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write("<u128>");
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{}", self));
    }
}

/**
i8 is probably not private.
*/

impl Loggable for i8 {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{}", self));
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        self.log_redacting_private_info(builder);
    }
}

/**
i16 might be private.
*/

impl Loggable for i16 {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write("<i16>");
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{}", self));
    }
}

/**
i32 might be private.
*/

impl Loggable for i32 {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write("<i32>");
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{}", self));
    }
}

/**
i64 might be private.
*/

impl Loggable for i64 {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write("<i64>");
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{}", self));
    }
}

/**
i128 might be private.
*/

impl Loggable for i128 {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write("<i128>");
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{}", self));
    }
}

/**
f32 might be private.
*/

impl Loggable for f32 {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write("<f32>");
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{}", self));
    }
}

/**
f64 might be private.
*/

impl Loggable for f64 {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write("<f64>");
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{}", self));
    }
}

/**
bool is probably not private.
*/

impl Loggable for bool {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{}", self));
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        self.log_redacting_private_info(builder);
    }
}

/**
char is probably not private.
*/

impl Loggable for char {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{}", self));
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        self.log_redacting_private_info(builder);
    }
}

/**
String might be private.
*/

impl Loggable for String {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write("<String>");
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{:?}", self));
    }
}

/**
&str might be private.
*/

impl Loggable for &str {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write("<&str>");
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{:?}", self));
    }
}

/**
slices depend on the underlying type.
*/

impl<T: Loggable> Loggable for &[T] {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write("<[");
        for item in self.iter() {
            item.log_redacting_private_info(builder);
            builder.write(", ");
        }
        builder.write("]>");
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write("<[");
        for item in self.iter() {
            item.log_all(builder);
            builder.write(", ");
        }
        builder.write("]>");
    }
}

/**
Option depends on the underlying type.
*/

impl<T: Loggable> Loggable for Option<T> {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        match self {
            Some(t) => {
                builder.write("Some(");
                t.log_redacting_private_info(builder);
                builder.write(")");
            },
            None => builder.write("None"),
        }
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        match self {
            Some(t) => {
                builder.write("Some(");
                t.log_all(builder);
                builder.write(")");
            },
            None => builder.write("None"),
        }
    }
}

/**
Escape hatch, we won't send any data to servers.
*/

pub struct LogIt<T>(pub T);

impl<T: Debug> Loggable for LogIt<T> {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write("<LogIt>");
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        builder.write(&format!("{:?}", self.0));
    }
}


/**
Escape hatch, we promise we aren't sending private data to servers.
*/
pub struct IPromiseItsNotPrivate<T>(pub T);

impl<T: Loggable> Loggable for IPromiseItsNotPrivate<T> {
    #[inline]
    fn log_redacting_private_info<Builder: LogBuilder>(&self, builder: &mut Builder) {
        self.0.log_all(builder);
    }
    #[inline]
    fn log_all<Builder: LogBuilder>(&self, builder: &mut Builder) {
        self.0.log_all(builder);
    }
}