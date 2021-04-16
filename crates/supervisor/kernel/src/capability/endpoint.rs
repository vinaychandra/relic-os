use relic_abi::syscall::TaskBuffer;

use crate::arch::capability::paging::PageCap;

/// Task buffer page capability. Represents a page of task buffer.
pub type TaskBufferPageCap = PageCap<TaskBuffer>;
