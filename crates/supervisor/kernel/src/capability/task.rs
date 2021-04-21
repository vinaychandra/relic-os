use std::sync::atomic::AtomicU64;

use crate::{
    addr::VAddr,
    arch::{capability::TopPageTableCap, task::registers::Registers},
    capability::{CPoolCap, TaskBufferPageCap, UntypedDescriptor},
    util::managed_arc::{ManagedArc, ManagedArcAny, ManagedWeakPool3Arc},
};
use crossbeam_utils::atomic::AtomicCell;
use relic_abi::{cap::CapabilityErrors, syscall::SystemCall};
use spin::RwLock;

/// Represent a task status.
#[derive(Debug, Clone)]
pub enum TaskStatus {
    /// The task is running.
    Active,
    /// The task has never been started before.
    Inactive,

    /// The task has made a syscall and is now waiting for response.
    SyscalledAndWaiting(Option<SystemCall>),
    /// The task has made a syscall and is response is ready.
    /// Can optionally return upto two values.
    SyscalledReadyToResume(CapabilityErrors, u64, u64),

    /// Unknown task state.
    Unknown,
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Task capability. Reference-counted smart pointer to task
/// descriptor.
///
/// Tasks represents isolated processes running.
pub type TaskCap = ManagedArc<RwLock<TaskDescriptor>>;

impl TaskCap {
    /// Create a task capability from an untyped capability.
    pub fn retype_from(
        untyped: &mut UntypedDescriptor,
        priority: u8,
    ) -> Result<Self, CapabilityErrors> {
        let mut arc: Option<Self> = None;

        let weak_pool = unsafe {
            ManagedWeakPool3Arc::create(untyped.allocate(
                ManagedWeakPool3Arc::inner_type_length(),
                ManagedWeakPool3Arc::inner_type_alignment(),
            )?)
        };

        unsafe {
            untyped.derive(
                Self::inner_type_length(),
                Self::inner_type_alignment(),
                |paddr, next_child| {
                    arc = Some(Self::new(
                        paddr,
                        RwLock::new(TaskDescriptor {
                            weak_pool,
                            priority,
                            task_id: TASK_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
                            runtime: Registers::default(),
                            next: next_child,
                            next_task: None,
                            status: TaskStatus::Inactive,
                        }),
                    ));

                    arc.clone().unwrap()
                },
            )?
        };

        Ok(arc.unwrap())
    }
}

/// Task descriptor.
#[derive(Debug)]
pub struct TaskDescriptor {
    /// Contains 3 weak pointers
    /// 0: CPool, 1: PML4, 2: TaskBuffer
    weak_pool: ManagedWeakPool3Arc,

    /// Register state for the thread. Only valid
    /// when thread is not running.
    runtime: Registers,

    /// Next item in the memory tree.
    next: Option<ManagedArcAny>,

    /// Next task in the task list.
    next_task: Option<TaskCap>,

    status: TaskStatus,

    /// Priority of the task. Higher numbers are better priority.
    priority: u8,

    /// ID for the task.
    task_id: u64,
}

/// Simple task id generator.
static TASK_ID: AtomicU64 = AtomicU64::new(0);

impl TaskDescriptor {
    /// Set the task's instruction pointer.
    pub fn set_instruction_pointer(&mut self, instruction_pointer: VAddr) {
        self.runtime.set_rip(instruction_pointer.into());
    }

    /// Set the task's stack pointer.
    pub fn set_stack_pointer(&mut self, stack_pointer: VAddr) {
        self.runtime.set_rsp(stack_pointer.into());
    }

    /// Set the task's root capability pool.
    pub fn downgrade_cpool(&self, cpool: CPoolCap) {
        self.weak_pool.downgrade_at(cpool, 0).unwrap()
    }

    /// Read from the task's root capability pool.
    pub fn upgrade_cpool(&self) -> Option<CPoolCap> {
        self.weak_pool.upgrade(0)
    }

    /// Set the task's top page table.
    pub fn downgrade_top_page_table(&self, pml4: TopPageTableCap) {
        self.weak_pool.downgrade_at(pml4, 1).unwrap()
    }

    /// Read from the task's top page table.
    pub fn upgrade_top_page_table(&self) -> Option<TopPageTableCap> {
        self.weak_pool.upgrade(1)
    }

    /// Set the task's buffer.
    pub fn downgrade_buffer(&self, buffer: TaskBufferPageCap) {
        self.weak_pool.downgrade_at(buffer, 2).unwrap()
    }

    /// Read from the task's buffer.
    pub fn upgrade_buffer(&self) -> Option<TaskBufferPageCap> {
        self.weak_pool.upgrade(2)
    }

    /// Current task status.
    pub fn status(&self) -> TaskStatus {
        self.status.clone()
    }

    /// Set the current task status.
    pub fn set_status(&mut self, status: TaskStatus) {
        self.status = status;
    }

    /// Get priority for the task.
    pub fn get_priority(&self) -> u8 {
        self.priority
    }

    /// Set a new priority for the task.
    pub fn set_priority(&mut self, new_priority: u8) {
        assert!(new_priority < 16);
        self.priority = new_priority;
    }

    /// Set the tcb location for the task.
    pub fn set_tcb_location(&mut self, tcb: VAddr) {
        assert!(tcb.validate_user_mode().is_ok());
        assert_matches!(self.status, TaskStatus::Inactive);

        self.runtime.set_fs(tcb.into());
    }

    /// Switch to the task. The function is returned when exception
    /// happens.
    pub fn switch_to(&mut self) -> TaskStatus {
        // Mark this status as active.
        let mut current_status = TaskStatus::Active;
        core::mem::swap(&mut self.status, &mut current_status);

        if let Some(pml4) = self.upgrade_top_page_table() {
            pml4.write().switch_to();
        }

        let syscall_info = match current_status {
            TaskStatus::Inactive => Some((CapabilityErrors::None, 0, 0)),
            TaskStatus::SyscalledReadyToResume(a, b, c) => Some((a, b, c)),
            _ => None,
        };

        self.runtime.switch_to(syscall_info)
    }
}

/// The scheduler for the kernel. This contains 16 Priorities.
/// Each priority has two lists so that once run, a task is switched
/// between these two lists so that all tasks will be run.
/// Even indexed tasks are ready to run next. Odd indexed ones
/// will be moved to even ones when all even ones are done.
pub struct Scheduler {
    current_list: [AtomicCell<Option<TaskCap>>; 32],
}

impl Scheduler {
    pub const fn new() -> Self {
        const NONE_VAL: AtomicCell<Option<TaskCap>> = AtomicCell::new(None);

        Self {
            current_list: [NONE_VAL; 32],
        }
    }

    /// Add a task with the given priority.
    pub fn add_task_with_priority(&self, cap: TaskCap) {
        let mut cap_write = cap.write();

        let current_priority = cap_write.priority as usize;
        assert!(current_priority < 16);

        let latest = &self.current_list[current_priority * 2 + 1];
        cap_write.next_task = latest.take();

        core::mem::drop(cap_write);
        latest.store(Some(cap));
    }

    /// Get the next task to run.
    pub fn get_task_to_run(&self) -> Option<TaskCap> {
        for i in (0..=15usize).rev() {
            let mut current_queue_item = self.current_list[i * 2].take().take();
            if current_queue_item.is_none() {
                // Data is available in the other list. Lets use that instead.
                current_queue_item = self.current_list[i * 2 + 1].take().take();

                if current_queue_item.is_none() {
                    continue;
                }
            }

            if let Some(task_to_execute) = current_queue_item {
                let mut task_to_execute_writer = task_to_execute.write();
                let next_to_run = task_to_execute_writer.next_task.take();

                self.current_list[i * 2].store(next_to_run);

                core::mem::drop(task_to_execute_writer);
                return Some(task_to_execute);
            }
        }

        None
    }

    pub fn run_forever(&self) -> ! {
        loop {
            let task = self.get_task_to_run();
            if let Some(task_cap) = task {
                let task_status = task_cap.read().status();
                let result_status = match task_status {
                    TaskStatus::Inactive => task_cap.write().switch_to(),
                    TaskStatus::SyscalledReadyToResume(..) => task_cap.write().switch_to(),
                    default => panic!("Cannot run a task in '{:?}' state", default),
                };

                match result_status {
                    TaskStatus::SyscalledAndWaiting(data) => {
                        crate::syscall_processor::process_syscall(&task_cap, data, self)
                    }
                    default => panic!("Cannot result in this result state: {:?}", default),
                };
            } else {
                // Sleep
            }
        }
    }
}
