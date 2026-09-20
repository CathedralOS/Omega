//! Own the native process container from launch through closure and reaping.
//!
//! Explicit completion consumes the child only after closure and reaping;
//! dropping an unfinished child retains the platform cleanup fallback.

mod completion;
#[cfg(unix)]
mod descriptors;
pub(crate) mod limits;
#[cfg(windows)]
pub(crate) mod windows;

use crate::BoundedProcessPrepared;
pub use completion::{BoundedProcessCompletion, BoundedProcessExitStatus};
use std::io;
use std::process::{ChildStderr, ChildStdin, ChildStdout, ExitStatus};

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
        #[cfg(unix)]
        let retained_descriptors = prepared.retained_descriptors().to_vec();
        let (command, limits) = prepared.into_command()?;
        #[cfg(not(windows))]
        let _ = limits;
        #[cfg(unix)]
        let mut command = {
            let mut command = command;
            descriptors::mark_ambient_close_on_exec(&mut command)?;
            descriptors::retain_descriptors(&mut command, retained_descriptors)?;
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
            Err(error) if Self::group_signal_refused(&error) => {
                // macOS answers EPERM, not ESRCH, when every remaining member
                // of the group is an unreaped zombie (POSIX reserves EPERM for
                // a live process this caller may not signal). A primary that
                // exited before cleanup is exactly that case, so reap it and
                // retry once: a surviving descendant is then signalled by the
                // retry, an empty group answers ESRCH and closes, and a live
                // member this caller genuinely cannot signal still refuses.
                // A primary still in the middle of exiting is not reapable
                // yet and refuses again; the caller that owns the cleanup
                // budget polls and retries `terminate` in that case.
                self.try_wait()?;
                match self.child.kill() {
                    Ok(()) => {
                        self.container_closed = true;
                        Ok(())
                    }
                    Err(retry) if native_container_already_absent(&retry) => {
                        self.container_closed = true;
                        Ok(())
                    }
                    Err(_) => Err(error),
                }
            }
            Err(error) => Err(error),
        }
    }

    /// Whether a `terminate` failure means the platform refused to signal a
    /// group that still exists (POSIX EPERM). On macOS this is also what an
    /// exiting-but-not-yet-reapable primary answers, so it is worth retrying
    /// within the caller's cleanup budget rather than reporting at once.
    pub fn group_signal_refused(error: &io::Error) -> bool {
        #[cfg(unix)]
        {
            error.raw_os_error() == Some(1)
        }
        #[cfg(not(unix))]
        {
            let _ = error;
            false
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
        // the opposite: a group exists but this caller cannot signal it, or,
        // on macOS, its only members are zombies (see `terminate`).
        error.raw_os_error() == Some(3)
    }
    #[cfg(not(unix))]
    {
        error.kind() == io::ErrorKind::InvalidInput
    }
}

#[cfg(all(test, unix))]
mod tests;
