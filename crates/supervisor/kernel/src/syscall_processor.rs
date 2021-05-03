use relic_abi::{
    cap::CapabilityErrors,
    syscall::{SystemCall, TaskBuffer},
};

use crate::{
    addr::VAddr,
    capability::{CapAccessorMut, MapPermissions, Scheduler, StoredCap, Task, TaskStatus},
};

pub fn process_syscall(
    source_task: &mut CapAccessorMut<'_, Task>,
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

    let cpool_cap = source_task.cpool().clone().expect("CPool cannot be found");

    let syscall = syscall.unwrap();
    match syscall {
        SystemCall::Yield => {
            set_result_and_schedule(source_task, (CapabilityErrors::None, 0, 0), scheduler);
            return;
        }
        SystemCall::UntypedTotalFree(caddr) => {
            let result = || -> Result<(u64, u64), CapabilityErrors> {
                let cpool = cpool_cap.as_cpool()?;
                let untyped_data = cpool
                    .lookup(caddr)
                    .ok_or(CapabilityErrors::CapabilitySearchFailed)?;
                let untyped = untyped_data.as_untyped_memory()?;
                Ok((untyped.length() as u64, untyped.get_free_space() as u64))
            };

            match result() {
                Ok(r) => set_result_and_schedule(
                    source_task,
                    (CapabilityErrors::None, r.0, r.1),
                    scheduler,
                ),
                Err(e) => set_result_and_schedule(source_task, (e, 0, 0), scheduler),
            }
            return;
        }
        SystemCall::RawPageRetype {
            untyped_memory,
            size,
        } => {
            let result = || -> Result<(u64, u64), CapabilityErrors> {
                let mut cpool = cpool_cap.as_cpool_mut()?;
                let untyped_op = cpool
                    .lookup(untyped_memory)
                    .ok_or(CapabilityErrors::CapabilitySearchFailed)?;
                let mut untyped = untyped_op.as_untyped_memory_mut()?;
                let raw_page_cap = match size {
                    1 => StoredCap::large_page_retype_from::<[u8; 0x20_0000]>(
                        &mut untyped,
                        &mut cpool,
                        true,
                    )?,
                    2 => StoredCap::huge_page_retype_from::<[u8; 0x4000_0000]>(
                        &mut untyped,
                        &mut cpool,
                        true,
                    )?,
                    _ => StoredCap::base_page_retype_from::<[u8; 0x1000]>(
                        &mut untyped,
                        &mut cpool,
                        true,
                    )?,
                };
                Ok((raw_page_cap.1 as u64, 0u64))
            };

            match result() {
                Ok(r) => set_result_and_schedule(
                    source_task,
                    (CapabilityErrors::None, r.0, r.1),
                    scheduler,
                ),
                Err(e) => set_result_and_schedule(source_task, (e, 0, 0), scheduler),
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
                let mut cpool = cpool_cap.as_cpool_mut()?;
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
                let mut untyped = untyped_op.as_untyped_memory_mut()?;
                let perms = MapPermissions::WRITE | MapPermissions::EXECUTE;

                let mut top_level_table_mut = top_level_table.as_l4_mut().unwrap();
                top_level_table_mut.l4_map(vaddr, &raw_page, &mut untyped, &mut cpool, None, perms)
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
    task: &mut CapAccessorMut<Task>,
    result: (CapabilityErrors, u64, u64),
    scheduler: &Scheduler,
) {
    task.set_status(TaskStatus::SyscalledReadyToResume(
        result.0, result.1, result.2,
    ));
    scheduler.add_task_with_priority(task);
}

#[allow(dead_code)]
fn set_result_with_data_and_schedule<T>(
    task: &mut CapAccessorMut<Task>,
    mut result: (CapabilityErrors, u64, u64),
    data: T,
    scheduler: &Scheduler,
) {
    let buffer = task.task_buffer();
    if let Some(buf) = buffer {
        let mut b = buf.as_base_page_mut().unwrap();
        let buf = b.page_data_mut::<TaskBuffer>();
        buf.write_to_task_buffer(&data)
            .expect("Set result memory exceeded");
    } else {
        result = (CapabilityErrors::TaskBufferNotFound, 0, 0);
    }

    task.set_status(TaskStatus::SyscalledReadyToResume(
        result.0, result.1, result.2,
    ));

    scheduler.add_task_with_priority(task);
}
