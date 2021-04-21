use relic_abi::{cap::CapabilityErrors, syscall::SystemCall};

use crate::{
    addr::VAddr,
    arch::capability::TopPageTableCap,
    capability::{
        CPoolCap, MapPermissions, RawPageCap, Scheduler, TaskCap, TaskStatus, UntypedCap,
    },
};

pub fn process_syscall(source_task: &TaskCap, syscall: Option<SystemCall>, scheduler: &Scheduler) {
    if syscall.is_none() {
        set_result_and_schedule(
            source_task,
            (CapabilityErrors::SyscallNotFound, 0, 0),
            scheduler,
        );
        return;
    }

    let cpool: CPoolCap = source_task
        .read()
        .upgrade_cpool()
        .expect("CPool cannot be found!");

    let syscall = syscall.unwrap();
    match syscall {
        SystemCall::Yield => {
            set_result_and_schedule(source_task, (CapabilityErrors::None, 0, 0), scheduler);
            return;
        }
        SystemCall::UntypedTotalFree(caddr) => {
            let untyped_op: Option<UntypedCap> = cpool.lookup_upgrade(caddr);
            if let Some(untyped) = untyped_op {
                let data = (
                    CapabilityErrors::None,
                    *untyped.read().length() as u64,
                    untyped.read().get_free_space() as u64,
                );
                set_result_and_schedule(source_task, data, scheduler);
            } else {
                set_result_and_schedule(
                    source_task,
                    (CapabilityErrors::CapabilityMismatch, 0, 0),
                    scheduler,
                );
            }
            return;
        }
        SystemCall::RawPageRetype { untyped_memory } => {
            let untyped_op: Option<UntypedCap> = cpool.lookup_upgrade(untyped_memory);
            if let Some(untyped) = untyped_op {
                let mut result = CapabilityErrors::None;
                let mut cpool_index: u64 = 0;
                let raw_page_cap_result = RawPageCap::retype_from(&mut untyped.write());
                match raw_page_cap_result {
                    Ok(raw_page_cap) => match cpool.write().downgrade_any_free(raw_page_cap) {
                        Ok(index) => cpool_index = index as u64,
                        Err(e) => result = e,
                    },
                    Err(a) => result = a,
                }
                set_result_and_schedule(source_task, (result, cpool_index, 0), scheduler);
            } else {
                set_result_and_schedule(
                    source_task,
                    (CapabilityErrors::CapabilityMismatch, 0, 0),
                    scheduler,
                );
            }
        }
        SystemCall::RawPageMap {
            untyped_memory,
            top_level_table,
            vaddr,
            raw_page,
        } => {
            let func = move || -> Result<(), CapabilityErrors> {
                let raw_page: RawPageCap = cpool
                    .lookup_upgrade(raw_page)
                    .ok_or(CapabilityErrors::CapabilityMismatch)?;
                let mut top_level_table: TopPageTableCap = cpool
                    .lookup_upgrade(top_level_table)
                    .ok_or(CapabilityErrors::CapabilityMismatch)?;
                let vaddr: VAddr = vaddr.into();
                vaddr.validate_user_mode()?;
                let untyped_memory: UntypedCap = cpool
                    .lookup_upgrade(untyped_memory)
                    .ok_or(CapabilityErrors::CapabilityMismatch)?;
                let perms = MapPermissions::READ | MapPermissions::WRITE | MapPermissions::EXECUTE;
                top_level_table.map(
                    vaddr,
                    &raw_page,
                    &mut untyped_memory.write(),
                    &mut cpool.write(),
                    perms,
                )?;
                Ok(())
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
    task: &TaskCap,
    result: (CapabilityErrors, u64, u64),
    scheduler: &Scheduler,
) {
    task.write().set_status(TaskStatus::SyscalledReadyToResume(
        result.0, result.1, result.2,
    ));
    scheduler.add_task_with_priority(task.clone());
}

#[allow(dead_code)]
fn set_result_with_data_and_schedule<T>(
    task: &TaskCap,
    mut result: (CapabilityErrors, u64, u64),
    data: T,
    scheduler: &Scheduler,
) {
    let buffer = task.write().upgrade_buffer();
    if let Some(buf) = buffer {
        buf.write()
            .write()
            .write_to_task_buffer(&data)
            .expect("Set result memory exceeded");
    } else {
        result = (CapabilityErrors::TaskBufferNotFound, 0, 0);
    }

    task.write().set_status(TaskStatus::SyscalledReadyToResume(
        result.0, result.1, result.2,
    ));
    scheduler.add_task_with_priority(task.clone());
}
