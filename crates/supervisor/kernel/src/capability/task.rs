use core::ops::Deref;
use std::{cell::RefCell, ops::DerefMut, sync::atomic::AtomicU64};

use relic_abi::{cap::CapabilityErrors, syscall::SystemCall};

use crate::{
    addr::VAddr,
    arch::{capability::paging::L4, task::registers::Registers},
    capability::{
        BasePage, CapAccessorMut, Capability, CapabilityEnum, Cpool, StoredCap, UntypedMemory,
    },
    util::boxed::Boxed,
};

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

#[derive(Debug)]
pub struct Task {
    descriptor: Boxed<TaskDescriptor>,
    pub next_task_item: Option<StoredCap>,
    pub prev_task_item: Option<StoredCap>,
}

impl Deref for Task {
    type Target = TaskDescriptor;

    fn deref(&self) -> &Self::Target {
        &self.descriptor
    }
}

impl DerefMut for Task {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.descriptor
    }
}

#[derive(Debug, Getters, Setters)]
pub struct TaskDescriptor {
    #[getset(get = "pub")]
    cpool: Option<StoredCap>,
    top_level_table: Option<StoredCap>,
    #[getset(get = "pub")]
    task_buffer: Option<StoredCap>,

    /// Register state for the thread. Only valid
    /// when thread is not running.
    #[getset(get, set)]
    runtime: Registers,

    #[getset(get = "pub", set = "pub")]
    status: TaskStatus,

    #[getset(get = "pub", set = "pub")]
    priority: u8,

    task_id: u64,
}

static TASK_ID: AtomicU64 = AtomicU64::new(1);

impl StoredCap {
    pub fn task_retype_from(
        untyped: &mut UntypedMemory,
        cpool_to_store_in: &mut Cpool,
        priority: u8,
    ) -> Result<(StoredCap, usize), CapabilityErrors> {
        let mut result_index = 0;

        let location = untyped.derive(None, |task_desc| {
            unsafe {
                core::ptr::write(
                    task_desc,
                    TaskDescriptor {
                        task_id: TASK_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
                        priority,
                        status: TaskStatus::Inactive,
                        runtime: Registers::default(),
                        cpool: None,
                        top_level_table: None,
                        task_buffer: None,
                    },
                )
            };

            let boxed = unsafe { Boxed::new((task_desc as u64).into()) };
            let cpool_location_to_store = cpool_to_store_in.get_free_index()?;

            let location = cpool_to_store_in.write_to_if_empty(
                cpool_location_to_store,
                Capability {
                    capability_data: CapabilityEnum::Task(Task {
                        descriptor: boxed,
                        next_task_item: None,
                        prev_task_item: None,
                    }),
                    ..Default::default()
                },
            )?;

            result_index = cpool_location_to_store;
            Ok(location)
        })?;

        Ok((location, result_index))
    }
}

impl CapAccessorMut<'_, Task> {
    pub fn task_set_cpool(
        &mut self,
        cap: &mut CapAccessorMut<'_, Cpool>,
    ) -> Result<(), CapabilityErrors> {
        if self.cpool.is_some() {
            Err(CapabilityErrors::CapabilityAlreadyOccupied)?
        }
        if cap.linked_task.is_some() {
            Err(CapabilityErrors::CapabilityAlreadyOccupied)?
        }

        cap.linked_task = Some(self.cap().clone());
        self.cpool = Some(cap.cap().clone());
        Ok(())
    }

    pub fn task_set_top_level_table(
        &mut self,
        l4: &mut CapAccessorMut<'_, L4>,
    ) -> Result<(), CapabilityErrors> {
        if self.top_level_table.is_some() {
            Err(CapabilityErrors::CapabilityAlreadyOccupied)?
        }
        if l4.linked_task.is_some() {
            Err(CapabilityErrors::CapabilityAlreadyOccupied)?
        }

        l4.linked_task = Some(self.cap().clone());
        self.top_level_table = Some(l4.cap().clone());

        Ok(())
    }

    pub fn task_set_task_buffer(
        &mut self,
        task_buffer: &mut CapAccessorMut<'_, BasePage>,
    ) -> Result<(), CapabilityErrors> {
        if self.task_buffer.is_some() {
            Err(CapabilityErrors::CapabilityAlreadyOccupied)?
        }

        if task_buffer.linked_task.is_some() {
            Err(CapabilityErrors::CapabilityAlreadyOccupied)?
        }

        task_buffer.linked_task = Some(self.cap().clone());

        self.task_buffer = Some(task_buffer.cap().clone());
        Ok(())
    }
}

impl TaskDescriptor {
    /// Set the task's instruction pointer.
    pub fn set_instruction_pointer(&mut self, instruction_pointer: VAddr) {
        self.runtime.set_rip(instruction_pointer.into());
    }

    /// Set the task's stack pointer.
    pub fn set_stack_pointer(&mut self, stack_pointer: VAddr) {
        self.runtime.set_rsp(stack_pointer.into());
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

        if let Some(pml4) = self.top_level_table.clone() {
            let mut l4 = pml4
                .as_l4_mut()
                .expect("Task's top level page table is not PML4");
            l4.switch_to();
        } else {
            panic!("Cannot start task without pml4");
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
    current_list: [RefCell<Capability>; 32],
}

impl Scheduler {
    pub const fn new() -> Self {
        const REFCELL_MARKER_TASK: RefCell<Capability> = RefCell::new(Capability {
            capability_data: CapabilityEnum::Task(Task {
                descriptor: unsafe { Boxed::new_unchecked(0xFFFF_FFFF_DEAD_DEAD) },
                next_task_item: None,
                prev_task_item: None,
            }),
            next_mem_item: None,
            prev_mem_item: None,
        });
        Self {
            current_list: [REFCELL_MARKER_TASK; 32],
        }
    }

    /// Add a task with the given priority.
    pub fn add_task_with_priority(&self, new_task: &mut CapAccessorMut<'_, Task>) {
        let task_priority = new_task.priority as usize;
        assert!(task_priority < 16);

        let current_list = &self.current_list[task_priority * 2 + 1];
        new_task.prev_task_item = unsafe { Some(StoredCap::from_raw(current_list)) };

        let mut list_accessor = current_list.borrow_mut();
        let to_be_second = list_accessor.get_next_task_item_mut().take();
        *list_accessor.get_next_task_item_mut() = Some(new_task.cap().clone());

        new_task.next_task_item = to_be_second.clone();
        if let Some(to_be_second_val) = to_be_second {
            *to_be_second_val
                .as_ref()
                .borrow_mut()
                .get_prev_task_item_mut() = Some(new_task.cap().clone());
        }
    }

    /// Get the next task to run.
    pub fn get_task_to_run(&self) -> Option<StoredCap> {
        for i in (0..=15usize).rev() {
            let mut current_queue_item = self.current_list[i * 2]
                .borrow_mut()
                .get_next_task_item_mut()
                .take();
            if current_queue_item.is_none() {
                // Data may available in the other list. Lets use that instead.
                current_queue_item = self.current_list[i * 2 + 1]
                    .borrow_mut()
                    .get_next_task_item_mut()
                    .take();

                if current_queue_item.is_none() {
                    continue;
                }
            }

            if let Some(task_to_execute) = current_queue_item {
                let mut task_to_execute_writer = task_to_execute.as_task_mut().unwrap();
                let to_be_first = task_to_execute_writer.next_task_item.take();
                let cur = task_to_execute_writer.prev_task_item.take();
                debug_assert!(cur.is_some(), "This must be the 'root' of priority");
                if let Some(next) = to_be_first.clone() {
                    next.as_task_mut().unwrap().prev_task_item = cur;
                }

                *self.current_list[i * 2]
                    .borrow_mut()
                    .get_next_task_item_mut() = to_be_first;

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
                let mut desc = task_cap.as_task_mut().unwrap();
                let result_status = {
                    let task_status = desc.status.clone();

                    let result_status = match task_status {
                        TaskStatus::Inactive => desc.switch_to(),
                        TaskStatus::SyscalledReadyToResume(..) => desc.switch_to(),
                        default => panic!("Cannot run a task in '{:?}' state", default),
                    };
                    result_status
                };

                match result_status {
                    TaskStatus::SyscalledAndWaiting(data) => {
                        crate::syscall_processor::process_syscall(&mut desc, data, self)
                    }
                    default => panic!("Cannot result in this result state: {:?}", default),
                };
            } else {
                // Sleep
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::mem::MaybeUninit;

    use crate::{addr::PAddrGlobal, capability::CpoolInner};

    use super::*;

    #[test]
    fn test_scheduler() {
        let raw_memory: Box<MaybeUninit<[u8; 0x20_0000 * 5]>> = Box::new_uninit();
        let raw_addr = Box::into_raw(raw_memory) as u64;
        let addr = PAddrGlobal::new(raw_addr);

        let mut untyped_memory = unsafe { UntypedMemory::bootstrap(addr, 0x20_0000 * 5) };
        const NONE_INNER: RefCell<Capability> = RefCell::new(Capability::new());
        let root_cpool_inner = CpoolInner {
            unsafe_data: [NONE_INNER; 256],
        };
        let mut root_cpool = Cpool {
            linked_task: None,
            data: unsafe {
                Boxed::new(PAddrGlobal::new(
                    &root_cpool_inner as *const CpoolInner as u64,
                ))
            },
        };

        if let CapabilityEnum::UntypedMemory(untyped) = &mut untyped_memory.capability_data {
            let scheduler = Scheduler::new();

            let task1 = StoredCap::task_retype_from(untyped, &mut root_cpool, 5).unwrap();
            let mut task1_0 = task1.0.as_task_mut().unwrap();
            assert!(task1_0.descriptor.task_id == 1);
            task1_0.descriptor.priority = 5;

            let task2 = StoredCap::task_retype_from(untyped, &mut root_cpool, 5).unwrap();
            let mut task2_0 = task2.0.as_task_mut().unwrap();
            assert!(task2_0.descriptor.task_id == 2);
            task2_0.descriptor.priority = 5;

            let task3 = StoredCap::task_retype_from(untyped, &mut root_cpool, 5).unwrap();
            let mut task3_0 = task3.0.as_task_mut().unwrap();
            assert!(task3_0.descriptor.task_id == 3);
            task3_0.descriptor.priority = 10;

            scheduler.add_task_with_priority(&mut task1_0);
            scheduler.add_task_with_priority(&mut task3_0);
            scheduler.add_task_with_priority(&mut task2_0);

            let next_task = scheduler.get_task_to_run().unwrap();
            let next_task_val = next_task.as_task_mut().unwrap();
            assert_eq!(3, next_task_val.descriptor.task_id);

            let next_task = scheduler.get_task_to_run().unwrap();
            let mut next_task_val = next_task.as_task_mut().unwrap();
            assert_eq!(2, next_task_val.descriptor.task_id);
            scheduler.add_task_with_priority(&mut next_task_val);

            let next_task = scheduler.get_task_to_run().unwrap();
            let mut next_task_val = next_task.as_task_mut().unwrap();
            assert_eq!(1, next_task_val.descriptor.task_id);
            scheduler.add_task_with_priority(&mut next_task_val);

            let next_task = scheduler.get_task_to_run().unwrap();
            let next_task_val = next_task.as_task_mut().unwrap();
            assert_eq!(1, next_task_val.descriptor.task_id);
        }
    }
}
