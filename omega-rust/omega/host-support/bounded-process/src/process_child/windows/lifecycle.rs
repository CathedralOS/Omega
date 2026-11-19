// `WindowsJobChild`: lifecycle owner for a child process and its Windows Job
// Object.
//
// `try_wait` returns `Some(ExitStatus)` only after both the primary child
// process has exited (`primary_status.is_some()`) and the job's completion port
// has delivered `JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO`
// (`active_processes_zero == true`), guaranteeing no descendant processes
// outlive the wait. Dropping `WindowsJobChild` terminates the job and reaps
// the primary process handle.

use std::io;
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus};
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{FALSE, WAIT_TIMEOUT};
use windows_sys::Win32::System::IO::GetQueuedCompletionStatus;
use windows_sys::Win32::System::SystemServices::JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO;
#[cfg(test)]
use windows_sys::Win32::System::SystemServices::{
    JOB_OBJECT_MSG_ACTIVE_PROCESS_LIMIT, JOB_OBJECT_MSG_END_OF_JOB_TIME,
    JOB_OBJECT_MSG_JOB_MEMORY_LIMIT, JOB_OBJECT_MSG_PROCESS_MEMORY_LIMIT,
};
use windows_sys::Win32::System::Threading::CREATE_SUSPENDED;

use super::job::{
    associate_completion_port, configure_job_limits, configure_tool_job, create_completion_port,
    create_job,
};
use super::launch::assign_and_resume;
use super::limits::WindowsJobLimits;
use super::native::{KernelHandle, raw_os_error_is};
use super::termination::{terminate_and_reap_child, terminate_job};
use crate::BoundedProcessLimits;

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum JobLimitEvent {
    ActiveProcess,
    ProcessMemory,
    AggregateMemory,
    AggregateCpu,
}

#[derive(Debug)]
pub(crate) struct WindowsJobChild {
    child: Child,                       // Primary child process handle.
    job: KernelHandle, // Compiler-created Windows Job Object owning the process tree.
    completion_port: KernelHandle, // I/O completion port receiving job notifications.
    primary_status: Option<ExitStatus>, // Latched exit status of the primary child once reaped.
    active_processes_zero: bool, // True once `JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO` arrives.
    #[cfg(test)]
    pub(super) limit_events: Vec<JobLimitEvent>,
}

impl WindowsJobChild {
    pub(crate) fn resource_usage(&self) -> crate::ProcessResourceUsage {
        use std::os::windows::io::AsRawHandle;
        super::super::resource_usage::windows_usage(self.child.as_raw_handle())
    }
    pub(crate) fn id(&self) -> u32 {
        self.child.id()
    }

    pub(crate) fn spawn(command: &mut Command, limits: BoundedProcessLimits) -> io::Result<Self> {
        Self::spawn_with_config(command, WindowsJobLimits::from_limits(limits)?)
    }

    pub(super) fn spawn_with_config(
        command: &mut Command,
        limits: WindowsJobLimits,
    ) -> io::Result<Self> {
        let job = create_job()?;
        let completion_port = create_completion_port()?;

        configure_job_limits(&job, limits)?;
        Self::spawn_in_job(command, job, completion_port)
    }

    pub(crate) fn spawn_for_tool(command: &mut Command) -> io::Result<Self> {
        let job = create_job()?;
        let completion_port = create_completion_port()?;
        configure_tool_job(&job)?;
        Self::spawn_in_job(command, job, completion_port)
    }

    fn spawn_in_job(
        command: &mut Command,
        job: KernelHandle,
        completion_port: KernelHandle,
    ) -> io::Result<Self> {
        Self::spawn_in_job_with(command, job, completion_port, assign_and_resume)
    }

    fn spawn_in_job_with(
        command: &mut Command,
        job: KernelHandle,
        completion_port: KernelHandle,
        assign: impl FnOnce(&mut Child, &KernelHandle) -> io::Result<()>,
    ) -> io::Result<Self> {
        associate_completion_port(&job, &completion_port)?;

        command.creation_flags(CREATE_SUSPENDED);
        let mut child = command.spawn()?;

        if let Err(setup_error) = assign(&mut child, &job) {
            terminate_and_reap_child(&mut child, &job);
            return Err(setup_error);
        }

        Ok(Self {
            child,
            job,
            completion_port,
            primary_status: None,
            active_processes_zero: false,
            #[cfg(test)]
            limit_events: Vec::new(),
        })
    }

    pub(crate) fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub(crate) fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }

    pub(crate) fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    pub(crate) fn kill(&mut self) -> io::Result<()> {
        terminate_job(&self.job)
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.drain_job_notifications()?;
        self.try_wait_primary()?;

        if self.active_processes_zero {
            Ok(self.primary_status)
        } else {
            Ok(None)
        }
    }

    pub(crate) fn try_wait_primary(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.primary_status.is_none() {
            self.primary_status = self.child.try_wait()?;
        }
        Ok(self.primary_status)
    }

    fn drain_job_notifications(&mut self) -> io::Result<()> {
        loop {
            let mut message = 0_u32;
            let mut completion_key = 0_usize;
            let mut overlapped = null_mut();
            let result = unsafe {
                GetQueuedCompletionStatus(
                    self.completion_port.raw(),
                    &mut message,
                    &mut completion_key,
                    &mut overlapped,
                    0,
                )
            };

            if result == FALSE {
                let error = io::Error::last_os_error();
                if raw_os_error_is(&error, WAIT_TIMEOUT) && overlapped.is_null() {
                    return Ok(());
                }
                return Err(error);
            }

            if completion_key != self.job.raw() as usize {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows Job Object completion key did not match its owner",
                ));
            }

            if message == JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO {
                self.active_processes_zero = true;
            }
            #[cfg(test)]
            if let Some(event) = job_limit_event(message)
                && !self.limit_events.contains(&event)
            {
                self.limit_events.push(event);
            }
        }
    }
}

#[cfg(test)]
fn job_limit_event(message: u32) -> Option<JobLimitEvent> {
    match message {
        JOB_OBJECT_MSG_ACTIVE_PROCESS_LIMIT => Some(JobLimitEvent::ActiveProcess),
        JOB_OBJECT_MSG_PROCESS_MEMORY_LIMIT => Some(JobLimitEvent::ProcessMemory),
        JOB_OBJECT_MSG_JOB_MEMORY_LIMIT => Some(JobLimitEvent::AggregateMemory),
        JOB_OBJECT_MSG_END_OF_JOB_TIME => Some(JobLimitEvent::AggregateCpu),
        _ => None,
    }
}

impl Drop for WindowsJobChild {
    fn drop(&mut self) {
        terminate_and_reap_child(&mut self.child, &self.job);
    }
}

#[cfg(test)]
mod startup_tests {
    use super::*;
    use std::os::windows::io::{AsHandle, AsRawHandle};
    use std::process::Stdio;
    use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
    use windows_sys::Win32::System::Threading::WaitForSingleObject;

    #[test]
    fn failed_tool_assignment_stops_and_reaps_suspended_child() {
        let job = create_job().unwrap();
        configure_tool_job(&job).unwrap();
        let port = create_completion_port().unwrap();
        let mut command = Command::new("cmd.exe");
        command
            .args(["/D", "/S", "/C", "exit /b 0"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut observed = None;
        let result = WindowsJobChild::spawn_in_job_with(&mut command, job, port, |child, _| {
            observed = Some(child.as_handle().try_clone_to_owned()?);
            Err(io::Error::other("injected assignment failure"))
        });
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("injected assignment failure")
        );
        let handle = observed.unwrap();
        // SAFETY: this owned observation handle remains valid after the spawn
        // error path closes its original child handle. A signalled process
        // object proves the suspended child was stopped before returning.
        assert_eq!(
            unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) },
            WAIT_OBJECT_0
        );
    }
}
