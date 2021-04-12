//! Architecture specific package. For each architecture,
//! this module is flattened into arch module.

#[cfg(target_arch = "x86_64")]
#[macro_use]
mod x86_64;

#[cfg(target_arch = "x86_64")]
pub use self::x86_64::*;