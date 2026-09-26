//! Sealed Git command execution: policy, launch accounting, bounded capture,
//! and whole-resolution budget reconciliation.

pub(crate) mod capture;
mod policy;
pub(crate) mod reconciliation;
#[cfg(test)]
mod tests;

use self::capture::run_command_capture;
use self::policy::sealed_git_command;
use self::reconciliation::reconcile_git_command_result;
use crate::bounded_process::{BoundedProcessInput, BoundedProcessOutput};
use crate::package_source::SourceResolveError;
use crate::package_source::git::executable::executor::GitExecutor;
use crate::package_source::limits::{GIT_STDERR_LIMIT, GIT_STDOUT_LIMIT};
use crate::resolver_execution::ResolverExecutionPhase;
use std::ffi::OsStr;
use std::path::Path;

/// Per-operation capture inputs. Deadlines, stderr bounds, and cumulative
/// accounting remain owned by the command lifecycle, not its callers.
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

/// Returns the actual exit status: object protocols must distinguish a
/// successful absence response from a failed command themselves.
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
