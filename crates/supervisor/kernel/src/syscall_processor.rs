use relic_abi::{cap::CapabilityErrors, syscall::SystemCall};

use crate::capability::{CPoolCap, Scheduler, TaskCap, TaskStatus, UntypedCap};

pub fn process_syscall(source_task: &TaskCap, syscall: Option<SystemCall>, scheduler: &Scheduler) {
    if syscall.is_none() {
        set_result_and_schedule(source_task, CapabilityErrors::SyscallNotFound, scheduler);
        return;
    }

    let cpool: CPoolCap = source_task
        .read()
        .upgrade_cpool()
        .expect("CPool cannot be found!");

    let syscall = syscall.unwrap();
    match syscall {
        SystemCall::Yield => {
            set_result_and_schedule(source_task, CapabilityErrors::None, scheduler);
            return;
        }
        SystemCall::UntypedTotalFree(caddr) => {
            let untyped_op: Option<UntypedCap> = cpool.lookup_upgrade(caddr);
            if let Some(untyped) = untyped_op {
                let result = CapabilityErrors::None;
                let data = (*untyped.read().length(), untyped.read().get_free_space());
                set_result_with_data_and_schedule(source_task, result, data, scheduler);
            } else {
                let result = CapabilityErrors::CapabilityMismatch;
                set_result_and_schedule(source_task, result, scheduler);
            }
            return;
        }
        _ => {
            // This should never really happen.
            set_result_and_schedule(source_task, CapabilityErrors::Unknown, scheduler);
            return;
        }
    }
}

fn set_result_and_schedule(task: &TaskCap, result: CapabilityErrors, scheduler: &Scheduler) {
    task.write()
        .set_status(TaskStatus::SyscalledReadyToResume(result));
    scheduler.add_task_with_priority(task.clone());
}

fn set_result_with_data_and_schedule<T>(
    task: &TaskCap,
    mut result: CapabilityErrors,
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
        result = CapabilityErrors::TaskBufferNotFound;
    }

    task.write()
        .set_status(TaskStatus::SyscalledReadyToResume(result));
    scheduler.add_task_with_priority(task.clone());
}
