#[cfg(unix)]
use super::descriptors;
use super::{BoundedProcessCompletion, BoundedProcessExitStatus};
use crate::BoundedProcessPrepared;
use std::io;
use std::process::{ChildStderr, ChildStdin, ChildStdout, ExitStatus};

#[cfg(windows)]
use super::windows;
#[cfg(unix)]
use command_group::CommandGroup;

pub struct BoundedProcessChild {
    #[cfg(unix)]
    child: command_group::GroupChild,
    #[cfg(windows)]
    child: windows::WindowsJobChild,
    #[cfg(not(any(unix, windows)))]
    child: std::process::Child,
    container_closed: bool,
    status: Option<ExitStatus>,
    finished: bool,
}

impl BoundedProcessChild {
    /// Spawn a configured command inside the platform process
    /// container before any child code may execute.
    pub fn spawn(prepared: BoundedProcessPrepared) -> io::Result<Self> {
        let (command, limits) = prepared.into_command()?;
        #[cfg(not(windows))]
        let _ = limits;
        #[cfg(unix)]
        let mut command = {
            let mut command = command;
            descriptors::mark_ambient_close_on_exec(&mut command)?;
            command
        };
        #[cfg(windows)]
        let mut command = command;
        #[cfg(windows)]
        let child = windows::WindowsJobChild::spawn(&mut command, limits)?;
        #[cfg(unix)]
        let child = command.group_spawn()?;
        #[cfg(not(any(unix, windows)))]
        let child = {
            let mut command = command;
            command.spawn()?
        };
        Ok(Self {
            child,
            container_closed: false,
            status: None,
            finished: false,
        })
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        #[cfg(windows)]
        return self.child.take_stdout();
        #[cfg(not(windows))]
        self.inner().stdout.take()
    }

    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        #[cfg(windows)]
        return self.child.take_stdin();
        #[cfg(not(windows))]
        self.inner().stdin.take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        #[cfg(windows)]
        return self.child.take_stderr();
        #[cfg(not(windows))]
        self.inner().stderr.take()
    }

    /// Terminate the platform-owned process container. This is a process group
    /// on Unix and a Job Object on Windows. Calling it after natural primary
    /// exit intentionally closes descendants that remain in that container.
    pub fn terminate(&mut self) -> io::Result<()> {
        match self.child.kill() {
            Ok(()) => {
                self.container_closed = true;
                Ok(())
            }
            Err(error) if native_container_already_absent(&error) => {
                self.container_closed = true;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let status = self.child.try_wait()?;
        if let Some(status) = status {
            self.status = Some(status);
        }
        Ok(status)
    }

    /// Consume a closed and reaped bounded execution.
    pub fn finish(mut self) -> io::Result<BoundedProcessCompletion> {
        if !self.container_closed {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "bounded process container was not explicitly closed",
            ));
        }
        let status = self.status.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                "bounded process was not reaped before completion",
            )
        })?;
        self.finished = true;
        Ok(BoundedProcessCompletion::new(
            BoundedProcessExitStatus::from_status(status),
        ))
    }

    #[cfg(unix)]
    fn inner(&mut self) -> &mut std::process::Child {
        self.child.inner()
    }

    #[cfg(not(any(unix, windows)))]
    fn inner(&mut self) -> &mut std::process::Child {
        &mut self.child
    }
}

impl Drop for BoundedProcessChild {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        #[cfg(windows)]
        let _ = self.child.kill();
        #[cfg(not(windows))]
        {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn native_container_already_absent(error: &io::Error) -> bool {
    #[cfg(unix)]
    {
        // POSIX ESRCH alone proves that no process group exists. EPERM proves
        // the opposite: a group exists but this caller cannot signal it.
        error.raw_os_error() == Some(3)
    }
    #[cfg(not(unix))]
    {
        error.kind() == io::ErrorKind::InvalidInput
    }
}
