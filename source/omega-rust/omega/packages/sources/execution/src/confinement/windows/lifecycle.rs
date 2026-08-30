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
    associate_completion_port, configure_job_limits, create_completion_port, create_job,
};
use super::launch::assign_and_resume;
use super::limits::WindowsJobLimits;
use super::native::{KernelHandle, raw_os_error_is};
use super::termination::{terminate_and_reap_child, terminate_job};

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum JobLimitEvent {
    ActiveProcess,
    ProcessMemory,
    AggregateMemory,
    AggregateCpu,
}

/// A resolver child contained in a compiler-owned Windows Job Object.
#[derive(Debug)]
pub(crate) struct WindowsJobChild {
    child: Child,
    job: KernelHandle,
    completion_port: KernelHandle,
    primary_status: Option<ExitStatus>,
    active_processes_zero: bool,
    #[cfg(test)]
    pub(super) limit_events: Vec<JobLimitEvent>,
}

impl WindowsJobChild {
    pub(crate) fn spawn(command: &mut Command) -> io::Result<Self> {
        Self::spawn_with_config(command, WindowsJobLimits::production()?)
    }

    pub(super) fn spawn_with_config(
        command: &mut Command,
        limits: WindowsJobLimits,
    ) -> io::Result<Self> {
        let job = create_job()?;
        let completion_port = create_completion_port()?;

        configure_job_limits(&job, limits)?;
        associate_completion_port(&job, &completion_port)?;

        command.creation_flags(CREATE_SUSPENDED);
        let mut child = command.spawn()?;

        if let Err(setup_error) = assign_and_resume(&mut child, &job) {
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

        if self.primary_status.is_none() {
            self.primary_status = self.child.try_wait()?;
        }

        if self.active_processes_zero {
            Ok(self.primary_status)
        } else {
            Ok(None)
        }
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
