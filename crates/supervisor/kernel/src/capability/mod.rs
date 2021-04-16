/// Capability pool capability implementation.
mod cpool;
/// Endpoint Capability implementations.
mod endpoint;
/// Threading support.
mod task;
/// Untyped capability implementation.
mod untyped;

pub use cpool::*;
pub use endpoint::*;
pub use task::*;
pub use untyped::*;

use relic_abi::SetDefault;

use crate::arch::capability::paging::{PageCap, PAGE_LENGTH};

/// Raw page struct representing a whole page.
pub struct RawPage(pub [u8; PAGE_LENGTH]);
/// Raw page capability. Represents a page with no other information.
pub type RawPageCap = PageCap<RawPage>;

impl std::fmt::Debug for RawPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawPage").finish_non_exhaustive()
    }
}

impl SetDefault for RawPage {
    fn set_default(&mut self) {
        for raw in self.0.iter_mut() {
            *raw = 0x0;
        }
    }
}

bitflags! {
    /// Permissions for the current page.
    pub struct MapPermissions : u8 {
        const READ      = 0b0000_0000;
        const WRITE     = 0b0000_0010;
        const EXECUTE   = 0b0000_0100;
    }
}
