#![cfg_attr(not(test), no_std)]
#![feature(coerce_unsized)]
#![feature(negative_impls)]
#![feature(unsize)]

extern crate core as std;

#[macro_use]
extern crate relic_utils;

/// Support for addresses.
pub mod addr;

/// Utilities for the kernel.
pub mod util;

/// Prelude to re-upload commonly used items.
pub mod prelude {
    pub use crate::util::memory_object::MemoryObject;
}
