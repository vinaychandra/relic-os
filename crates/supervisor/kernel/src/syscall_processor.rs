use relic_abi::{
    cap::CapabilityErrors,
    syscall::{SystemCall, TaskBuffer},
};

use crate::{
    addr::VAddr,
    capability::{MapPermissions, Scheduler, StoredCap, TaskStatus},
};

pub fn process_syscall(
    source_task: &StoredCap,
    syscall: Option<SystemCall>,
    scheduler: &Scheduler,
) {
    if syscall.is_none() {
        set_result_and_schedule(
            source_task,
            (CapabilityErrors::SyscallNotFound, 0, 0),
            scheduler,
        );
        return;
    }

    let cpool: StoredCap = source_task
        .task_create_mut(|t| {
            Ok(t.descriptor
                .get_or_insert_with(|| unreachable!())
                .cpool()
                .clone()
                .expect("CPool cannot be found"))
        })
        .unwrap();

    let syscall = syscall.unwrap();
    match syscall {
        SystemCall::Yield => {
            set_result_and_schedule(source_task, (CapabilityErrors::None, 0, 0), scheduler);
            return;
        }
        SystemCall::UntypedTotalFree(caddr) => {
            let result = || -> Result<(u64, u64), CapabilityErrors> {
                cpool.cpool_create(|cpool| {
                    let untyped_op = cpool.lookup(caddr);
                    if let Some(untyped_data) = untyped_op {
                        untyped_data.untyped_memory_create(|untyped| {
                            Ok((untyped.length() as u64, untyped.get_free_space() as u64))
                        })
                    } else {
                        Err(CapabilityErrors::CapabilityMismatch)
                    }
                })
            };

            match result() {
                std::result::Result::Ok(r) => set_result_and_schedule(
                    source_task,
                    (CapabilityErrors::None, r.0, r.1),
                    scheduler,
                ),
                std::result::Result::Err(e) => {
                    set_result_and_schedule(source_task, (e, 0, 0), scheduler)
                }
            }
            return;
        }
        SystemCall::RawPageRetype { untyped_memory } => {
            let result = || -> Result<(u64, u64), CapabilityErrors> {
                cpool.cpool_create_mut(|cpool| {
                    let untyped_op = cpool.lookup(untyped_memory);
                    if let Some(untyped_cap) = untyped_op {
                        untyped_cap.untyped_memory_create_mut(|untyped| {
                            let raw_page_cap =
                                StoredCap::base_page_retype_from::<[u8; 0x1000]>(untyped, cpool)?;
                            Ok((raw_page_cap.1 as u64, 0u64))
                        })
                    } else {
                        Err(CapabilityErrors::CapabilityMismatch)
                    }
                })
            };

            match result() {
                std::result::Result::Ok(r) => set_result_and_schedule(
                    source_task,
                    (CapabilityErrors::None, r.0, r.1),
                    scheduler,
                ),
                std::result::Result::Err(e) => {
                    set_result_and_schedule(source_task, (e, 0, 0), scheduler)
                }
            }
            return;
        }
        SystemCall::RawPageMap {
            untyped_memory,
            top_level_table,
            vaddr,
            raw_page,
        } => {
            let func = move || -> Result<(), CapabilityErrors> {
                cpool.cpool_create_mut(|cpool| {
                    let raw_page = cpool
                        .lookup(raw_page)
                        .ok_or(CapabilityErrors::CapabilitySearchFailed)?;
                    let top_level_table = cpool
                        .lookup(top_level_table)
                        .ok_or(CapabilityErrors::CapabilitySearchFailed)?;

                    let vaddr: VAddr = vaddr.into();
                    vaddr.validate_user_mode()?;
                    let untyped_op = cpool
                        .lookup(untyped_memory)
                        .ok_or(CapabilityErrors::CapabilitySearchFailed)?;
                    untyped_op.untyped_memory_create_mut(|untyped| {
                        let perms = MapPermissions::WRITE | MapPermissions::EXECUTE;
                        top_level_table.l4_map(vaddr, &raw_page, untyped, cpool, perms)
                    })
                })
            };
            let data = func().err().unwrap_or(CapabilityErrors::None);
            set_result_and_schedule(source_task, (data, 0, 0), scheduler);
            return;
        }
        SystemCall::None => {
            // This should never really happen.
            set_result_and_schedule(source_task, (CapabilityErrors::Unknown, 0, 0), scheduler);
            return;
        }
        _ => todo!("Syscall not implemented"),
    }
}

fn set_result_and_schedule(
    task: &StoredCap,
    result: (CapabilityErrors, u64, u64),
    scheduler: &Scheduler,
) {
    task.task_create_mut(|task_write| {
        task_write
            .descriptor
            .get_or_insert_with(|| unreachable!())
            .set_status(TaskStatus::SyscalledReadyToResume(
                result.0, result.1, result.2,
            ));
        Ok(())
    })
    .unwrap();
    scheduler.add_task_with_priority(task.clone());
}

#[allow(dead_code)]
fn set_result_with_data_and_schedule<T>(
    task: &StoredCap,
    mut result: (CapabilityErrors, u64, u64),
    data: T,
    scheduler: &Scheduler,
) {
    task.task_create_mut(|task_write| {
        let buffer = task_write
            .descriptor
            .get_or_insert_with(|| unreachable!())
            .task_buffer();
        if let Some(buf) = buffer {
            buf.base_page_create_mut(|b| {
                let buf = b.page_data_mut::<TaskBuffer>();
                buf.write_to_task_buffer(&data)
                    .expect("Set result memory exceeded");
                Ok(())
            })
            .unwrap();
        } else {
            result = (CapabilityErrors::TaskBufferNotFound, 0, 0);
        }

        task_write
            .descriptor
            .get_or_insert_with(|| unreachable!())
            .set_status(TaskStatus::SyscalledReadyToResume(
                result.0, result.1, result.2,
            ));
        Ok(())
    })
    .unwrap();
    scheduler.add_task_with_priority(task.clone());
}
