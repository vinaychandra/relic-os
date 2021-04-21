use relic_abi::{cap::CapabilityErrors, syscall::SystemCall};

use crate::capability::{CPoolCap, Scheduler, TaskCap, TaskStatus, UntypedCap};

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
        _ => {
            // This should never really happen.
            set_result_and_schedule(source_task, (CapabilityErrors::Unknown, 0, 0), scheduler);
            return;
        }
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
