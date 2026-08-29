//! Git-specific invocation over compiler-owned endpoint routes.

use super::capture::{BoundedCommandOutput, run_command_bounded_with_budget};
use super::command::sealed_git_command_with_route;
use super::identity::{GitCommandStdinIdentity, git_command_configuration_identity};
use super::reconciliation::reconcile_git_command_endpoint_result;
use crate::source::acquisition::SourceResolveError;
use crate::source::acquisition::git::execution::executable::GitExecutor;
use crate::source::acquisition::limits::{GIT_STDERR_LIMIT, GIT_STDOUT_LIMIT};
use omega_resolver_execution::{
    ResolverExecutionEndpointObservation, ResolverExecutionEndpointOutcome,
    ResolverExecutionEndpointRoute, ResolverExecutionPhase,
};
use std::ffi::OsStr;
use std::path::Path;

pub(in crate::source::acquisition) fn run_git<I, S>(
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

pub(in crate::source::acquisition) fn run_git_stdout<I, S>(
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

pub(in crate::source::acquisition) fn run_git_bytes_stdout<I, S>(
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

pub(in crate::source::acquisition) fn run_git_output<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
    args: I,
) -> Result<BoundedCommandOutput, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let endpoint_route = if matches!(
        phase,
        ResolverExecutionPhase::TransportDiscovery | ResolverExecutionPhase::Fetch
    ) {
        Some(
            executor
                .execution_backend
                .open_endpoint_route(
                    executor.requested_network_endpoint.clone(),
                    executor.network_transfer_budget.clone(),
                )
                .map_err(|error| SourceResolveError::GitExecutionBoundaryInvalid {
                    message: format!("cannot open the compiler-owned endpoint route: {error}"),
                })?,
        )
    } else {
        None
    };
    let mut command =
        sealed_git_command_with_route(executor, working_directory, phase, endpoint_route.as_ref())?;
    let command_timeout = executor.begin_launch()?;
    command.args(args);
    let command_identity =
        git_command_configuration_identity(&command, phase, &GitCommandStdinIdentity::Null);
    let result = run_command_bounded_with_budget(
        &mut command,
        "command",
        GIT_STDOUT_LIMIT,
        GIT_STDERR_LIMIT,
        command_timeout,
        executor.captured_output_budget.clone(),
    );
    let endpoint_result = endpoint_route
        .map(ResolverExecutionEndpointRoute::finish)
        .transpose()
        .map_err(|error| SourceResolveError::GitExecutionBoundaryInvalid {
            message: format!("compiler-owned endpoint route failed: {error}"),
        });
    let endpoint_validation = endpoint_result
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|observation| validate_network_transfer_outcome(observation.as_ref()));
    let output = reconcile_git_command_endpoint_result(
        result,
        endpoint_validation,
        executor.verify(),
        executor.verify_budget(),
    )?;
    let endpoint_observation =
        endpoint_result.expect("successful reconciliation checked endpoint result");
    executor.record_command_execution(phase, command_identity, &output, endpoint_observation)?;
    Ok(output)
}

fn validate_network_transfer_outcome(
    observation: Option<&ResolverExecutionEndpointObservation>,
) -> Result<(), SourceResolveError> {
    let Some(observation) = observation else {
        return Ok(());
    };
    if observation
        .events()
        .iter()
        .any(|event| event.outcome() == ResolverExecutionEndpointOutcome::TransferCeilingReached)
    {
        return Err(SourceResolveError::GitResolutionNetworkTransferCeiling {
            ceiling: observation.route().transfer_byte_ceiling(),
        });
    }
    Ok(())
}
