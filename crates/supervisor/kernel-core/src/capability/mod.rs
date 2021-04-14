/// Capability pool capability implementation.
mod cpool;
/// Untyped capability implementation.
mod untyped;

pub use cpool::*;
pub use untyped::*;

use relic_abi::{cap::TaskBuffer, SetDefault};

use crate::arch::capability::paging::{PageCap, PAGE_LENGTH};

/// Raw page struct representing a whole page.
pub struct RawPage(pub [u8; PAGE_LENGTH]);
/// Raw page capability. Represents a page with no other information.
pub type RawPageCap = PageCap<RawPage>;
/// Task buffer page capability. Represents a page of task buffer.
pub type TaskBufferPageCap = PageCap<TaskBuffer>;

impl SetDefault for RawPage {
    fn set_default(&mut self) {
        for raw in self.0.iter_mut() {
            *raw = 0x0;
        }
    }
}
