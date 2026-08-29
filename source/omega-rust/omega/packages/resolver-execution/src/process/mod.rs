use std::io;

pub(crate) mod limits;
mod observation;

pub use observation::{
    ResolverExecutionCompletionObservation, ResolverExecutionExitStatus,
    ResolverExecutionTerminationDisposition,
};

use crate::{
    ResolverExecutionCommandIdentity, ResolverExecutionPolicyObservation, ResolverPreparedExecution,
};
use std::process::{ChildStderr, ChildStdin, ChildStdout, ExitStatus};

#[cfg(any(target_os = "linux", windows))]
use crate::confinement;
#[cfg(all(unix, not(target_os = "linux")))]
use command_group::CommandGroup;

pub struct ResolverExecutionChild {
    #[cfg(unix)]
    child: command_group::GroupChild,
    #[cfg(windows)]
    child: confinement::windows::WindowsJobChild,
    #[cfg(not(any(unix, windows)))]
    child: std::process::Child,
    policy: Option<ResolverExecutionPolicyObservation>,
    command: ResolverExecutionCommandIdentity,
    termination: Option<ResolverExecutionTerminationDisposition>,
    status: Option<ExitStatus>,
}

impl ResolverExecutionChild {
    /// Spawn a configured resolver command inside the platform process
    /// container before any child code may execute.
    pub fn spawn(prepared: ResolverPreparedExecution) -> io::Result<Self> {
        let command_identity = prepared.command_identity()?;
        let (command, policy) = prepared.into_parts();
        #[cfg(target_os = "linux")]
        let child = confinement::linux::spawn(command, &policy)?;
        #[cfg(all(unix, not(target_os = "linux")))]
        let child = {
            let mut command = command;
            command.group_spawn()?
        };
        #[cfg(windows)]
        let child = {
            let mut command = command;
            confinement::windows::WindowsJobChild::spawn(&mut command)?
        };
        #[cfg(not(any(unix, windows)))]
        let child = {
            let mut command = command;
            command.spawn()?
        };
        Ok(Self {
            child,
            policy: Some(policy),
            command: command_identity,
            termination: None,
            status: None,
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

    /// Close the entire native process container. Calling this after natural
    /// primary-process exit is intentional: it prevents surviving descendants
    /// from being detached from the completion observation.
    pub fn terminate(&mut self) -> io::Result<()> {
        match self.child.kill() {
            Ok(()) => {
                self.termination = Some(ResolverExecutionTerminationDisposition::Requested);
                Ok(())
            }
            Err(error) if native_container_already_absent(&error) => {
                self.termination = Some(ResolverExecutionTerminationDisposition::AlreadyAbsent);
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

    /// Consume a fully closed and reaped resolver execution and issue its
    /// lifecycle-bound completion observation.
    pub fn finish(mut self) -> io::Result<ResolverExecutionCompletionObservation> {
        let termination = self.termination.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "resolver execution container was not explicitly closed",
            )
        })?;
        let status = self.status.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                "resolver execution was not reaped before completion",
            )
        })?;
        let policy = self
            .policy
            .take()
            .expect("resolver execution policy is consumed exactly once");
        Ok(ResolverExecutionCompletionObservation::new(
            policy,
            self.command,
            termination,
            ResolverExecutionExitStatus::from_status(status),
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

fn native_container_already_absent(error: &io::Error) -> bool {
    #[cfg(unix)]
    {
        // POSIX ESRCH alone proves that no process group exists. EPERM proves
        // the opposite: a group exists but this resolver cannot signal it.
        error.raw_os_error() == Some(3)
    }
    #[cfg(not(unix))]
    {
        error.kind() == io::ErrorKind::InvalidInput
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::{ResolverExecutionBackend, ResolverExecutionPhase};
    use std::path::Path;
    use std::time::{Duration, Instant};

    #[test]
    fn spawn_rejects_implicit_inherited_standard_streams() {
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let inspection_root = std::env::temp_dir()
            .canonicalize()
            .expect("canonical temporary root");
        let mut prepared = backend
            .prepare_inspection(Path::new("/usr/bin/true"), &[], &inspection_root)
            .expect("prepare inspection execution");
        prepared.env_clear().current_dir(&inspection_root);

        let error = ResolverExecutionChild::spawn(prepared)
            .err()
            .expect("implicit standard streams must be rejected");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("explicitly null or piped"));
    }

    #[test]
    fn command_identity_binds_closed_standard_stream_dispositions() {
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let inspection_root = std::env::temp_dir()
            .canonicalize()
            .expect("canonical temporary root");
        let prepare = |piped_stdout: bool| {
            let mut prepared = backend
                .prepare_inspection(Path::new("/usr/bin/true"), &[], &inspection_root)
                .expect("prepare inspection execution");
            prepared
                .env_clear()
                .current_dir(&inspection_root)
                .stdin_null()
                .stderr_null();
            if piped_stdout {
                prepared.stdout_piped();
            } else {
                prepared.stdout_null();
            }
            prepared
                .command_identity()
                .expect("identify closed standard streams")
        };

        assert_ne!(prepare(false), prepare(true));
    }

    #[test]
    fn completion_binds_prepared_command_policy_termination_and_reaping() {
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let inspection_root = std::env::temp_dir()
            .canonicalize()
            .expect("canonical temporary root");
        let mut prepared = backend
            .prepare_inspection(Path::new("/usr/bin/true"), &[], &inspection_root)
            .expect("prepare inspection execution");
        prepared
            .env_clear()
            .current_dir(&inspection_root)
            .stdin_null()
            .stdout_null()
            .stderr_null();
        let command = prepared
            .command_identity()
            .expect("identify prepared command");
        let mut child = ResolverExecutionChild::spawn(prepared).expect("spawn prepared execution");
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = child.try_wait().expect("poll prepared execution") {
                break status;
            }
            assert!(Instant::now() < deadline, "prepared execution timed out");
            std::thread::sleep(Duration::from_millis(5));
        };
        assert!(status.success());
        child.terminate().expect("close native process container");
        child.try_wait().expect("confirm reaped execution");
        let completion = child.finish().expect("issue completion observation");

        assert_eq!(completion.command(), command);
        assert_eq!(
            completion.policy().phase(),
            ResolverExecutionPhase::RepositoryInspection
        );
        assert!(completion.status().success());
        assert!(!completion.canonical_bytes().is_empty());
    }

    #[test]
    fn unfinished_execution_cannot_issue_completion() {
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let inspection_root = std::env::temp_dir()
            .canonicalize()
            .expect("canonical temporary root");
        let mut prepared = backend
            .prepare_inspection(Path::new("/usr/bin/true"), &[], &inspection_root)
            .expect("prepare inspection execution");
        prepared
            .env_clear()
            .current_dir(&inspection_root)
            .stdin_null()
            .stdout_null()
            .stderr_null();
        let mut child = ResolverExecutionChild::spawn(prepared).expect("spawn prepared execution");
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait().expect("poll prepared execution").is_none() {
            assert!(Instant::now() < deadline, "prepared execution timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(child.finish().is_err());
    }
}
