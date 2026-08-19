#![no_std]

pub fn facade_is_linkable() {
    let _ = core::mem::size_of::<Option<core::convert::Infallible>>();
}
