#![cfg_attr(not(test), no_std)]

pub mod bootstrap;
pub mod caddr;
pub mod cap;
pub mod syscall;

/// Prelude to re-upload commonly used items.
pub mod prelude {
    pub use crate::caddr::CAddr;
}

/// A trait that allows resetting a struct back to its default value.
pub trait SetDefault {
    /// Reset this struct back to its default value.
    fn set_default(&mut self);
}
