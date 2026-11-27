// Sealed Git command execution: policy configuration, launch accounting,
// bounded stream capture, and whole-resolution budget reconciliation.
//
// `GitCommandCapture` supplies per-operation capture inputs while deadlines,
// stderr bounds, and cumulative output accounting remain owned by the command
// lifecycle. `run_git_output` returns the raw `BoundedProcessOutput` so object
// inspection protocols can distinguish a clean absence response from a failed
// process, whereas `run_git`, `run_git_stdout`, and `run_git_bytes_stdout`
// require a zero exit status.

mod policy;
pub(crate) mod reconciliation;
#[cfg(test)]
mod tests;

use self::policy::sealed_git_command;
use self::reconciliation::reconcile_git_command_result;
use crate::SourceResolveError;
use crate::git::executable::{GitCapturedOutputBudget, GitExecutor};
use crate::git::execution::ResolverExecutionPhase;
use crate::limits::{
    GIT_COMMAND_CLEANUP_TIMEOUT, GIT_STDERR_LIMIT, GIT_STDOUT_LIMIT, PROCESS_POLL_INTERVAL,
};
use bounded_process::BoundedProcessPrepared;
use bounded_process::{
    BoundedCaptureLimits, BoundedProcessInput, BoundedProcessOutput, BoundedProcessRunError,
    run_bounded_process,
};
use std::ffi::OsStr;
use std::path::Path;
use std::time::Duration;

pub(crate) struct GitCommandCapture<'a> {
    pub(crate) operation: &'a str,
    pub(crate) input: BoundedProcessInput,
    pub(crate) stdout_limit: usize,
}

impl Default for GitCommandCapture<'_> {
    fn default() -> Self {
        Self {
            operation: "command",
            input: BoundedProcessInput::Null,
            stdout_limit: GIT_STDOUT_LIMIT,
        }
    }
}

pub(crate) fn run_git_output<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
    arguments: I,
    capture: GitCommandCapture<'_>,
) -> Result<BoundedProcessOutput, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = sealed_git_command(executor, working_directory, phase)?;
    let deadline = executor.begin_launch()?;
    command.args(arguments);
    let result = run_command_capture(
        command,
        capture.input,
        capture.operation,
        capture.stdout_limit,
        GIT_STDERR_LIMIT,
        deadline.duration(),
        executor.captured_output_budget.clone(),
    )
    .map_err(|error| deadline.project_error(error));
    reconcile_git_command_result(result, executor.verify_budget())
}

pub(crate) fn run_git<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
    arguments: I,
) -> Result<(), SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_git_bytes_stdout(executor, working_directory, phase, arguments).map(|_| ())
}

pub(crate) fn run_git_stdout<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
    arguments: I,
) -> Result<String, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let stdout = run_git_bytes_stdout(executor, working_directory, phase, arguments)?;
    Ok(String::from_utf8_lossy(&stdout).into_owned())
}

pub(crate) fn run_git_bytes_stdout<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
    arguments: I,
) -> Result<Vec<u8>, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_git_output(
        executor,
        working_directory,
        phase,
        arguments,
        GitCommandCapture::default(),
    )?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(SourceResolveError::Git {
            operation: "command".to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

// Bounded child-process capture and `BoundedProcessRunError` to
// `SourceResolveError` projection for sealed Git commands.

#[cfg(all(test, unix))]
pub(crate) fn run_command_bounded(
    command: BoundedProcessPrepared,
    operation: &str,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
) -> Result<BoundedProcessOutput, SourceResolveError> {
    run_command_bounded_with_budget(
        command,
        operation,
        stdout_limit,
        stderr_limit,
        timeout,
        GitCapturedOutputBudget::new(u64::MAX),
    )
}

#[cfg(all(test, unix))]
pub(crate) fn run_command_bounded_with_budget(
    command: BoundedProcessPrepared,
    operation: &str,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
    captured_output_budget: GitCapturedOutputBudget,
) -> Result<BoundedProcessOutput, SourceResolveError> {
    run_command_capture(
        command,
        BoundedProcessInput::Null,
        operation,
        stdout_limit,
        stderr_limit,
        timeout,
        captured_output_budget,
    )
}

fn run_command_capture(
    command: BoundedProcessPrepared,
    input: BoundedProcessInput,
    operation: &str,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
    captured_output_budget: GitCapturedOutputBudget,
) -> Result<BoundedProcessOutput, SourceResolveError> {
    let limits = BoundedCaptureLimits::new(
        stdout_limit,
        stderr_limit,
        timeout,
        GIT_COMMAND_CLEANUP_TIMEOUT,
        PROCESS_POLL_INTERVAL,
    );
    run_bounded_process(command, input, limits, captured_output_budget)
        .map_err(|error| project_error(operation, error))
}

pub(in crate::git::git_command) fn project_error(
    operation: &str,
    error: BoundedProcessRunError,
) -> SourceResolveError {
    match error {
        BoundedProcessRunError::Spawn(message) => SourceResolveError::Git {
            operation: format!("{operation} spawn"),
            status: None,
            stderr: message,
        },
        BoundedProcessRunError::WorkerSpawn { worker, message } => SourceResolveError::Git {
            operation: format!("{operation} {worker}"),
            status: None,
            stderr: message,
        },
        BoundedProcessRunError::StreamCapture { stream, message } => SourceResolveError::Git {
            operation: format!("{operation} {} capture", stream.name()),
            status: None,
            stderr: message,
        },
        BoundedProcessRunError::InputTransfer(message) => SourceResolveError::Git {
            operation: format!("{operation} stdin transfer"),
            status: None,
            stderr: message,
        },
        BoundedProcessRunError::OutputOverflow { stream, limit } => {
            SourceResolveError::GitOutputOverflow {
                operation: operation.to_owned(),
                stream: stream.name().to_owned(),
                limit,
            }
        }
        BoundedProcessRunError::AggregateOutputOverflow { ceiling, attempted } => {
            SourceResolveError::GitResolutionCapturedOutputLimit { ceiling, attempted }
        }
        BoundedProcessRunError::TimedOut { timeout } => SourceResolveError::GitTimedOut {
            operation: operation.to_owned(),
            timeout_millis: duration_millis(timeout),
        },
        BoundedProcessRunError::Wait(message) => SourceResolveError::Git {
            operation: format!("{operation} wait"),
            status: None,
            stderr: message,
        },
        BoundedProcessRunError::Cleanup(message) => SourceResolveError::GitCleanupFailed {
            operation: operation.to_owned(),
            message,
        },
        BoundedProcessRunError::Finalize(message) => {
            SourceResolveError::GitExecutionBoundaryInvalid {
                message: format!("cannot finalize resolver child execution: {message}"),
            }
        }
        BoundedProcessRunError::InvalidLimits => SourceResolveError::GitExecutionBoundaryInvalid {
            message: "bounded process received invalid capture limits".to_owned(),
        },
        BoundedProcessRunError::WorkersEndedEarly => SourceResolveError::Git {
            operation: format!("{operation} capture"),
            status: None,
            stderr: "command stream workers ended before all transfers completed".to_owned(),
        },
    }
}

pub(crate) fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
