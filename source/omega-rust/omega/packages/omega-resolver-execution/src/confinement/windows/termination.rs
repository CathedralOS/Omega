use std::io;
use std::process::Child;

use windows_sys::Win32::System::JobObjects::TerminateJobObject;

use super::native::{KernelHandle, bool_result};

const JOB_TERMINATION_EXIT_CODE: u32 = 1;

pub(super) fn terminate_job(job: &KernelHandle) -> io::Result<()> {
    bool_result(unsafe { TerminateJobObject(job.raw(), JOB_TERMINATION_EXIT_CODE) })
}

pub(super) fn terminate_and_reap_child(child: &mut Child, job: &KernelHandle) {
    let _ = terminate_job(job);
    let _ = child.kill();
    let _ = child.wait();
}
