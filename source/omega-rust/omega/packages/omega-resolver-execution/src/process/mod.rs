use std::io;

pub(crate) mod limits;
use std::process::{ChildStderr, ChildStdout, Command, ExitStatus};

#[cfg(windows)]
use crate::confinement::windows;
#[cfg(unix)]
use command_group::CommandGroup;

pub struct ResolverExecutionChild {
    #[cfg(unix)]
    child: command_group::GroupChild,
    #[cfg(windows)]
    child: windows::WindowsJobChild,
    #[cfg(not(any(unix, windows)))]
    child: std::process::Child,
}

impl ResolverExecutionChild {
    /// Spawn a configured resolver command inside the platform process
    /// container before any child code may execute.
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        #[cfg(unix)]
        let child = command.group_spawn()?;
        #[cfg(windows)]
        let child = windows::WindowsJobChild::spawn(command)?;
        #[cfg(not(any(unix, windows)))]
        let child = command.spawn()?;
        Ok(Self { child })
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        #[cfg(windows)]
        return self.child.take_stdout();
        #[cfg(not(windows))]
        self.inner().stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        #[cfg(windows)]
        return self.child.take_stderr();
        #[cfg(not(windows))]
        self.inner().stderr.take()
    }

    pub fn kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
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
