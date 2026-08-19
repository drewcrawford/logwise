// SPDX-License-Identifier: MIT OR Apache-2.0

#![no_std]
#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

//! The facade API is being introduced incrementally during the 0.7 rewrite.
//! Until a dispatcher is installed by a runtime, every facade operation is a
//! no-op by contract.

#[cfg(feature = "alloc")]
extern crate alloc;

mod context;
mod dispatch;
mod macros;
mod metadata;
mod value;

pub use context::ContextToken;
pub use dispatch::{Callsite, Dispatch, InstallError, Interest, install_dispatcher};
pub use metadata::{
    Class, Detail, Domain, FieldMetadata, Kind, Location, Metadata, Privacy, Severity,
};
pub use value::{EventRef, FieldRef, ValueRef};
