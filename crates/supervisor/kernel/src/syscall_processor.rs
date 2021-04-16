use relic_abi::{cap::CapabilityErrors, syscall::SystemCall};

use crate::capability::{Scheduler, TaskCap, TaskStatus};

pub fn process_syscall(source_task: &TaskCap, syscall: Option<SystemCall>, scheduler: &Scheduler) {
    if syscall.is_none() {
        set_result_and_schedule(source_task, CapabilityErrors::SyscallNotFound, scheduler);
        return;
    }

    let syscall = syscall.unwrap();
    match syscall {
        SystemCall::Yield => {
            set_result_and_schedule(source_task, CapabilityErrors::None, scheduler);
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
