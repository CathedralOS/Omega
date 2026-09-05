//! Git error projection over the shared bounded-process execution boundary.

use crate::SourceResolveError;
use crate::git::executable::budget::GitCapturedOutputBudget;
use crate::limits::{GIT_COMMAND_CLEANUP_TIMEOUT, PROCESS_POLL_INTERVAL};
use bounded_process::{
    BoundedCaptureLimits, BoundedProcessInput, BoundedProcessOutput, BoundedProcessRunError,
    run_bounded_process,
};
use resolver_execution::ResolverPreparedExecution;
use std::fs::File;
use std::time::Duration;

pub(crate) type BoundedCommandOutput = BoundedProcessOutput;

pub(crate) enum ResolverCommandInput {
    Null,
    File(File),
}

#[cfg(test)]
pub(crate) fn run_command_bounded(
    command: ResolverPreparedExecution,
    operation: &str,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
) -> Result<BoundedCommandOutput, SourceResolveError> {
    run_command_bounded_with_budget(
        command,
        operation,
        stdout_limit,
        stderr_limit,
        timeout,
        GitCapturedOutputBudget::new(u64::MAX),
    )
}

pub(crate) fn run_command_bounded_with_budget(
    command: ResolverPreparedExecution,
    operation: &str,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
    captured_output_budget: GitCapturedOutputBudget,
) -> Result<BoundedCommandOutput, SourceResolveError> {
    run_command_bounded_with_stdin_and_budget(
        command,
        ResolverCommandInput::Null,
        operation,
        stdout_limit,
        stderr_limit,
        timeout,
        captured_output_budget,
    )
}

pub(crate) fn run_command_bounded_with_stdin_and_budget(
    command: ResolverPreparedExecution,
    input: ResolverCommandInput,
    operation: &str,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
    captured_output_budget: GitCapturedOutputBudget,
) -> Result<BoundedCommandOutput, SourceResolveError> {
    let limits = BoundedCaptureLimits::new(
        stdout_limit,
        stderr_limit,
        timeout,
        GIT_COMMAND_CLEANUP_TIMEOUT,
        PROCESS_POLL_INTERVAL,
    );
    let input = match input {
        ResolverCommandInput::Null => BoundedProcessInput::Null,
        ResolverCommandInput::File(file) => BoundedProcessInput::File(file),
    };
    run_bounded_process(command, input, limits, captured_output_budget)
        .map_err(|error| project_error(operation, error))
}

#[cfg(all(test, unix))]
pub(crate) fn command_cleanup_reserve(timeout: Duration) -> Duration {
    GIT_COMMAND_CLEANUP_TIMEOUT.min(timeout / 4)
}

fn project_error(operation: &str, error: BoundedProcessRunError) -> SourceResolveError {
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
