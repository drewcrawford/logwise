// SPDX-License-Identifier: MIT OR Apache-2.0

#![no_std]
#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

//! The facade API is being introduced incrementally during the 0.7 rewrite.
//! Until a dispatcher is installed by a runtime, every facade operation is a
//! no-op by contract.

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod context;
mod dispatch;
mod macros;
mod metadata;
mod span;
mod value;

pub use context::link as link_context;
pub use context::{ContextGuard, ContextToken};
pub use context::{capture as capture_context, child as child_context, enter as enter_context};
pub use dispatch::{Callsite, Dispatch, InstallError, Interest, install_dispatcher};
pub use metadata::{
    Class, Detail, Domain, FieldMetadata, Kind, Location, Metadata, Privacy, Severity,
};
pub use span::{SpanGuard, SpanRef, SpanTiming, SpanToken};
pub use value::{EventRef, FieldRef, ValueRef};
