use std::{cell::RefCell, sync::atomic::AtomicU64};

use relic_abi::cap::CapabilityErrors;

use crate::{
    addr::VAddr,
    arch::task::registers::Registers,
    capability::TaskStatus,
    update::capability::{Capability, CapabilityEnum, Cpool, StoredCap, UntypedMemory},
    util::boxed::Boxed,
};

#[derive(Debug)]
pub struct Task {
    pub descriptor: Option<Boxed<TaskDescriptor>>,
    pub next_task_item: Option<StoredCap>,
    pub prev_task_item: Option<StoredCap>,
}

#[derive(Debug, Getters, Setters)]
#[getset(get = "pub", set = "pub")]
pub struct TaskDescriptor {
    cpool: Option<StoredCap>,
    top_level_table: Option<StoredCap>,
    task_buffer: Option<StoredCap>,

    /// Register state for the thread. Only valid
    /// when thread is not running.
    #[getset(get, set)]
    runtime: Registers,

    status: TaskStatus,

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
                        descriptor: Some(boxed),
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

impl TaskDescriptor {
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

        if let Some(pml4) = self.top_level_table() {
            pml4.l4_create_mut(|l4| {
                l4.switch_to();
                Ok(())
            })
            .expect("Task's top level page table is not PML4");
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
                descriptor: None,
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
    pub fn add_task_with_priority(&self, cap: StoredCap) {
        cap.task_create_mut(|new_task| {
            let task_priority = new_task
                .descriptor
                .get_or_insert_with(|| unreachable!())
                .priority as usize;
            assert!(task_priority < 16);

            let current_list = &self.current_list[task_priority * 2 + 1];
            new_task.prev_task_item = unsafe { Some(StoredCap::from_raw(current_list)) };

            let mut list_accessor = current_list.borrow_mut();
            let to_be_second = list_accessor.get_next_task_item_mut().take();
            *list_accessor.get_next_task_item_mut() = Some(cap.clone());

            new_task.next_task_item = to_be_second.clone();
            if let Some(to_be_second_val) = to_be_second {
                *to_be_second_val
                    .as_ref()
                    .borrow_mut()
                    .get_prev_task_item_mut() = Some(cap.clone());
            }

            Ok(())
        })
        .unwrap();
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
                task_to_execute
                    .task_create_mut(|task_to_execute_writer| {
                        let to_be_first = task_to_execute_writer.next_task_item.take();
                        let cur = task_to_execute_writer.prev_task_item.take();
                        debug_assert!(cur.is_some(), "This must be the 'root' of priority");
                        if let Some(next) = to_be_first.clone() {
                            next.task_create_mut(|t| {
                                t.prev_task_item = cur;
                                Ok(())
                            })
                            .unwrap();
                        }

                        *self.current_list[i * 2]
                            .borrow_mut()
                            .get_next_task_item_mut() = to_be_first;
                        Ok(())
                    })
                    .unwrap();

                return Some(task_to_execute);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use std::mem::MaybeUninit;

    use crate::{addr::PAddrGlobal, update::capability::CpoolInner};

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
            data: unsafe {
                Boxed::new(PAddrGlobal::new(
                    &root_cpool_inner as *const CpoolInner as u64,
                ))
            },
        };

        if let CapabilityEnum::UntypedMemory(untyped) = &mut untyped_memory.capability_data {
            let scheduler = Scheduler::new();

            let task1 = StoredCap::task_retype_from(untyped, &mut root_cpool, 5).unwrap();
            task1
                .0
                .task_create_mut(|t| {
                    assert!(t.descriptor.get_or_insert_with(|| unreachable!()).task_id == 1);
                    t.descriptor.get_or_insert_with(|| unreachable!()).priority = 5;
                    Ok(())
                })
                .unwrap();
            let task2 = StoredCap::task_retype_from(untyped, &mut root_cpool, 5).unwrap();
            task2
                .0
                .task_create_mut(|t| {
                    assert!(t.descriptor.get_or_insert_with(|| unreachable!()).task_id == 2);
                    t.descriptor.get_or_insert_with(|| unreachable!()).priority = 5;
                    Ok(())
                })
                .unwrap();
            let task3 = StoredCap::task_retype_from(untyped, &mut root_cpool, 5).unwrap();
            task3
                .0
                .task_create_mut(|t| {
                    assert!(t.descriptor.get_or_insert_with(|| unreachable!()).task_id == 3);
                    t.descriptor.get_or_insert_with(|| unreachable!()).priority = 10;
                    Ok(())
                })
                .unwrap();

            scheduler.add_task_with_priority(task1.0);
            scheduler.add_task_with_priority(task3.0);
            scheduler.add_task_with_priority(task2.0);

            let next_task = scheduler.get_task_to_run();
            assert!(next_task.is_some());
            assert_eq!(
                3,
                next_task
                    .unwrap()
                    .task_create_mut(|t| Ok(t
                        .descriptor
                        .get_or_insert_with(|| unreachable!())
                        .task_id))
                    .unwrap()
            );

            let next_task = scheduler.get_task_to_run().unwrap();
            assert_eq!(
                2,
                next_task
                    .task_create_mut(|t| Ok(t
                        .descriptor
                        .get_or_insert_with(|| unreachable!())
                        .task_id))
                    .unwrap()
            );
            scheduler.add_task_with_priority(next_task);

            let next_task = scheduler.get_task_to_run().unwrap();
            assert_eq!(
                1,
                next_task
                    .task_create_mut(|t| Ok(t
                        .descriptor
                        .get_or_insert_with(|| unreachable!())
                        .task_id))
                    .unwrap()
            );
            scheduler.add_task_with_priority(next_task);

            let next_task = scheduler.get_task_to_run().unwrap();
            assert_eq!(
                1,
                next_task
                    .task_create_mut(|t| Ok(t
                        .descriptor
                        .get_or_insert_with(|| unreachable!())
                        .task_id))
                    .unwrap()
            );
        }
    }
}
