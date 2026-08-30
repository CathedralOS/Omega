//! Resolve and authenticate one exact commit and root tree in a verified cache entry.

use crate::error::SourceResolveError;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::executor::GitExecutor;
use crate::git::objects::authentication::{authenticate_git_commit, verify_exact_git_revision};
use crate::git::request::GitExecutionTransport;
use crate::git::workspace::GitWorkspaceProjectionError;
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::{GitAcquisitionPin, PendingResolvedGitSource};
use cap_std::fs::Dir as CapabilityDirectory;
use std::ffi::OsStr;
use std::path::Path;

use super::materialization::GitMaterializedSource;
#[cfg(test)]
use super::materialization::materialize_whole_git_source;
use super::network::bounded_git_fetch_arguments;

#[cfg(test)]
pub(crate) fn resolve_verified_git_cache_entry(
    executor: &GitExecutor,
    cache_directory: &CapabilityDirectory,
    entry_name: &OsStr,
    entry_root: &Path,
    requested_locator: &str,
    locator_identity: &str,
    fetch_locator: &str,
    requested_rev: &str,
    execution_transport: GitExecutionTransport,
    limits: LocalSourceLimits,
    fetch_remote: bool,
) -> Result<PendingResolvedGitSource, SourceResolveError> {
    match resolve_verified_git_cache_entry_with(
        executor,
        cache_directory,
        entry_name,
        entry_root,
        requested_locator,
        locator_identity,
        fetch_locator,
        requested_rev,
        execution_transport,
        limits,
        fetch_remote,
        None,
        materialize_whole_git_source,
    ) {
        Ok((pending, ())) => Ok(pending),
        Err(GitWorkspaceProjectionError::Source(error)) => Err(error),
        Err(GitWorkspaceProjectionError::Planner(never)) => match never {},
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_verified_git_cache_entry_with<Evidence, PlannerError>(
    executor: &GitExecutor,
    cache_directory: &CapabilityDirectory,
    entry_name: &OsStr,
    entry_root: &Path,
    requested_locator: &str,
    locator_identity: &str,
    fetch_locator: &str,
    requested_rev: &str,
    execution_transport: GitExecutionTransport,
    limits: LocalSourceLimits,
    fetch_remote: bool,
    pin: Option<&GitAcquisitionPin>,
    materialize: impl FnOnce(
        &GitExecutor,
        &VerifiedGitRepository,
        &str,
        LocalSourceLimits,
    ) -> Result<
        GitMaterializedSource<Evidence>,
        GitWorkspaceProjectionError<PlannerError>,
    >,
) -> Result<(PendingResolvedGitSource, Evidence), GitWorkspaceProjectionError<PlannerError>> {
    let repository = VerifiedGitRepository::open(
        cache_directory,
        entry_name,
        entry_root,
        locator_identity,
        requested_rev,
        execution_transport,
        limits,
    )?;

    if fetch_remote {
        let canonical_config = repository.read_canonical_config()?;
        let arguments = bounded_git_fetch_arguments(fetch_locator, requested_rev, limits);
        repository.run_git(executor, arguments.iter())?;
        repository.restore_canonical_config(&canonical_config)?;
    }
    repository.verify_current(limits)?;

    let selected_revision = if let Some(pin) = pin {
        pin.commit()
    } else if fetch_remote {
        "FETCH_HEAD"
    } else {
        requested_rev
    };
    let commit = repository.run_git_stdout(
        executor,
        [
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new(&format!("{selected_revision}^{{commit}}")),
        ],
    )?;
    let commit = commit.trim().to_owned();
    verify_exact_git_revision(requested_rev, &commit)?;
    if pin.is_some_and(|pin| pin.commit() != commit) {
        return Err(SourceResolveError::GitObjectInvalid {
            oid: commit,
            message: "reused Git acquisition selected a different commit".to_owned(),
        }
        .into());
    }
    let tree = repository.run_git_stdout(
        executor,
        [
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new(&format!("{commit}^{{tree}}")),
        ],
    )?;
    let tree = tree.trim().to_owned();
    if pin.is_some_and(|pin| pin.tree() != tree) {
        return Err(SourceResolveError::GitObjectInvalid {
            oid: tree,
            message: "reused Git acquisition selected a different root tree".to_owned(),
        }
        .into());
    }
    repository.verify_current(limits)?;
    authenticate_git_commit(executor, &repository, &commit, &tree)?;
    let materialized = materialize(executor, &repository, &tree, limits)?;
    repository.verify_current(limits)?;
    executor.verify()?;
    executor.validate_execution_policy_observations()?;
    let pending = PendingResolvedGitSource {
        requested_locator: requested_locator.to_owned(),
        locator_identity: locator_identity.to_owned(),
        transport_profile: execution_transport.profile(),
        requested_rev: requested_rev.to_owned(),
        commit,
        materialized_tree: materialized.materialized_tree,
        tree,
        snapshot_root: materialized.snapshot_root,
        local: materialized.local,
        workspace_projection: materialized.workspace_projection,
        git_executable: executor.identity.clone(),
        transport_executable: executor
            .transport_executable
            .as_ref()
            .map(|executable| executable.identity.clone()),
        execution_helper_executables: executor
            .execution_helpers
            .iter()
            .map(|executable| executable.identity.clone())
            .collect(),
        execution_policy_observations: executor.execution_policy_observations.borrow().clone(),
        command_execution_observations: executor.command_execution_observations.borrow().clone(),
        captured_output_observation: executor.captured_output_observation()?,
        network_transfer_observation: executor.network_transfer_observation()?,
    };
    Ok((pending, materialized.evidence))
}
