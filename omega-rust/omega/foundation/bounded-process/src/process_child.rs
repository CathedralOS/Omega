/*
Native process-container lifecycle (`BoundedProcessChild`) from spawn through
container termination, reaping, and completion.

We place every spawned child inside an OS process container before any child
instruction runs (a Unix process group via `command-group` or a suspended-start
Windows Job Object via `windows::WindowsJobChild`). `finish` refuses to return a
`BoundedProcessCompletion` until both `terminate` (`container_closed == true`)
and `try_wait` (`status.is_some()`) have succeeded; dropping an unfinished
`BoundedProcessChild` kills and reaps the container as a fallback.

Why does `terminate` reap and retry once when `self.child.kill()` returns POSIX
`EPERM` (`group_signal_refused`)?
- POSIX `ESRCH` (errno 3) alone proves that no process group exists
  (`native_container_already_absent`).
- On macOS, `killpg` returns `EPERM` (errno 1) instead of `ESRCH` when every
  remaining member of the process group is an unreaped zombie. When the primary
  child exits before `terminate` is called, reaping it with `self.try_wait()?`
  and retrying `self.child.kill()` once either signals any surviving
  descendants (`Ok(())`), sees an empty group (`ESRCH` -> `Ok(())`), or
  preserves `EPERM` if a primary is still mid-exit so the caller's cleanup loop
  can poll and retry.
*/

mod completion;
mod resource_usage;
pub use resource_usage::ProcessResourceUsage;
#[cfg(unix)]
pub(crate) mod descriptors;
pub(crate) mod limits;
#[cfg(windows)]
pub(crate) mod windows;

use crate::BoundedProcessPrepared;
pub use completion::{BoundedProcessCompletion, BoundedProcessExitStatus};
use std::io;
use std::process::{ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus};

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
    observe_resources: bool,
    resource_usage: Option<ProcessResourceUsage>,
}

/// Containment for trusted build tools with caller-configured streams and
/// resource policy. A compiler build writes directly to retained files and may
/// wait indefinitely for shared scheduler capacity; runtime executions still
/// use the unchanged resource-capped `BoundedProcessPrepared` API.
///
/// The inner native owner is shared, including Windows suspended-start setup
/// failure cleanup. Primary exit is observable separately so a finished tool
/// wrapper cannot leave workers behind; completion still requires container
/// termination and the ordinary whole-container wait.
pub struct ToolCommandChild(BoundedProcessChild);

impl ToolCommandChild {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        #[cfg(unix)]
        let child = command.group_spawn()?;
        #[cfg(windows)]
        let child = windows::WindowsJobChild::spawn_for_tool(command)?;
        #[cfg(not(any(unix, windows)))]
        let child = command.spawn()?;
        Ok(Self(BoundedProcessChild {
            child,
            container_closed: false,
            status: None,
            finished: false,
            observe_resources: false,
            resource_usage: None,
        }))
    }

    pub fn id(&mut self) -> u32 {
        self.0.id()
    }

    /// Observe the tool wrapper's status, without waiting for its descendants.
    /// The capped owner's `try_wait` keeps its complete-job semantics.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        #[cfg(windows)]
        {
            let status = self.0.child.try_wait_primary()?;
            if let Some(status) = status {
                self.0.status = Some(status);
            }
            Ok(status)
        }
        #[cfg(not(windows))]
        self.0.try_wait()
    }

    pub fn terminate(&mut self) -> io::Result<()> {
        self.0.terminate()
    }

    /// Reap after termination, including Windows' active-process-zero receipt.
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        loop {
            if let Some(status) = self.0.try_wait()? {
                return Ok(status);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    pub fn finish(self) -> io::Result<BoundedProcessCompletion> {
        self.0.finish()
    }
}

impl BoundedProcessChild {
    pub fn id(&mut self) -> u32 {
        #[cfg(windows)]
        {
            self.child.id()
        }
        #[cfg(not(windows))]
        {
            self.inner().id()
        }
    }

    pub fn spawn(prepared: BoundedProcessPrepared) -> io::Result<Self> {
        let observe_resources = prepared.should_observe_resources();
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
            observe_resources,
            resource_usage: None,
        })
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        #[cfg(windows)]
        return self.child.take_stdout();
        #[cfg(not(windows))]
        self.inner().stdout.take()
    }

    pub(crate) fn take_stdin(&mut self) -> Option<ChildStdin> {
        #[cfg(windows)]
        return self.child.take_stdin();
        #[cfg(not(windows))]
        self.inner().stdin.take()
    }

    pub(crate) fn take_stderr(&mut self) -> Option<ChildStderr> {
        #[cfg(windows)]
        return self.child.take_stderr();
        #[cfg(not(windows))]
        self.inner().stderr.take()
    }

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

    pub(crate) fn group_signal_refused(error: &io::Error) -> bool {
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
        if self.observe_resources {
            if let Some(status) = self.status {
                return Ok(Some(status));
            }
            #[cfg(unix)]
            {
                if let Some((status, usage)) = resource_usage::try_wait_with_usage(self.id())? {
                    self.status = Some(status);
                    self.resource_usage = Some(usage);
                    return Ok(Some(status));
                }
                return Ok(None);
            }
        }
        let status = self.child.try_wait()?;
        if let Some(status) = status {
            self.status = Some(status);
            if self.observe_resources {
                #[cfg(windows)]
                {
                    self.resource_usage = Some(self.child.resource_usage());
                }
                #[cfg(not(any(unix, windows)))]
                {
                    self.resource_usage = Some(ProcessResourceUsage::unavailable(
                        "resource observations unsupported on this platform",
                    ));
                }
            }
        }
        Ok(status)
    }

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
            self.resource_usage.take(),
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
        error.raw_os_error() == Some(3)
    }
    #[cfg(not(unix))]
    {
        error.kind() == io::ErrorKind::InvalidInput
    }
}

#[cfg(all(test, unix))]
mod tests;
