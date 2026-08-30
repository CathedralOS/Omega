//! Git-specific invocation under bounded host-routed execution.

use super::capture::{BoundedCommandOutput, run_command_bounded_with_budget};
use super::command::sealed_git_command;
use super::identity::git_command_configuration_identity;
use super::reconciliation::reconcile_git_command_result;
use crate::SourceResolveError;
use crate::git::executable::executor::GitExecutor;
use crate::limits::{GIT_STDERR_LIMIT, GIT_STDOUT_LIMIT};
use crate::observations::execution::GitCommandInputCommitment;
use omega_resolver_execution::ResolverExecutionPhase;
use std::ffi::OsStr;
use std::path::Path;

pub(crate) fn run_git<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
    args: I,
) -> Result<(), SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_git_output(executor, working_directory, phase, args)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(SourceResolveError::Git {
            operation: "command".to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

pub(crate) fn run_git_stdout<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
    args: I,
) -> Result<String, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_git_output(executor, working_directory, phase, args)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(SourceResolveError::Git {
            operation: "command".to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

pub(crate) fn run_git_bytes_stdout<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
    args: I,
) -> Result<Vec<u8>, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_git_output(executor, working_directory, phase, args)?;
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

pub(crate) fn run_git_output<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
    args: I,
) -> Result<BoundedCommandOutput, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = sealed_git_command(executor, working_directory, phase)?;
    let command_timeout = executor.begin_launch()?;
    command.args(args);
    let input = GitCommandInputCommitment::Null;
    let command_identity = git_command_configuration_identity(&mut command, phase, &input)?;
    let result = run_command_bounded_with_budget(
        command,
        "command",
        GIT_STDOUT_LIMIT,
        GIT_STDERR_LIMIT,
        command_timeout,
        executor.captured_output_budget.clone(),
    );
    let output = reconcile_git_command_result(result, executor.verify(), executor.verify_budget())?;
    executor.record_command_execution(phase, command_identity, input, &output)?;
    Ok(output)
}
