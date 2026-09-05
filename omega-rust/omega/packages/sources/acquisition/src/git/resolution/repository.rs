//! Resolve and authenticate one exact commit and root tree in a verified cache entry.

use crate::error::SourceResolveError;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::executor::GitExecutor;
use crate::git::objects::authentication::{authenticate_git_commit, verify_exact_git_revision};
use crate::git::objects::identity::git_object_algorithm;
use crate::git::request::GitExecutionTransport;
use crate::git::workspace::GitWorkspaceProjectionError;
use crate::identity::SourceLineage;
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::PendingResolvedGitSource;
use cap_std::fs::Dir as CapabilityDirectory;
use std::ffi::OsStr;
use std::path::Path;

use super::materialization::GitMaterializedSource;
#[cfg(test)]
use super::materialization::materialize_whole_git_source;
use super::network::bounded_git_fetch_arguments;
use super::recorded_objects::recorded_revision_needs_fetch;
use super::selection::GitRevisionSelection;

#[cfg(test)]
pub(crate) fn resolve_verified_git_cache_entry(
    executor: &GitExecutor,
    cache_directory: &CapabilityDirectory,
    entry_name: &OsStr,
    entry_root: &Path,
    requested_locator: &str,
    lineage: &SourceLineage,
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
        lineage,
        locator_identity,
        fetch_locator,
        requested_rev,
        execution_transport,
        limits,
        fetch_remote,
        GitRevisionSelection::Ordinary(None),
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
    lineage: &SourceLineage,
    locator_identity: &str,
    fetch_locator: &str,
    requested_rev: &str,
    execution_transport: GitExecutionTransport,
    limits: LocalSourceLimits,
    fetch_remote: bool,
    selection: GitRevisionSelection<'_>,
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

    let fetch_remote = match selection {
        GitRevisionSelection::Recorded(recorded) => {
            recorded_revision_needs_fetch(executor, &repository, recorded, limits)?
        }
        GitRevisionSelection::Ordinary(_) => fetch_remote,
    };
    if fetch_remote {
        let canonical_config = repository.read_canonical_config()?;
        let fetch_revision = match selection {
            GitRevisionSelection::Recorded(recorded) => recorded.commit.as_str(),
            GitRevisionSelection::Ordinary(_) => requested_rev,
        };
        let arguments = bounded_git_fetch_arguments(fetch_locator, fetch_revision, limits);
        repository.run_git(executor, arguments.iter())?;
        repository.restore_canonical_config(&canonical_config)?;
    }
    repository.verify_current(limits)?;

    let selected_revision = if let Some(commit) = selection.expected_commit() {
        commit
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
    if selection
        .expected_commit()
        .is_some_and(|expected| expected != commit)
    {
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
    if selection
        .expected_tree()
        .is_some_and(|expected| expected != tree)
    {
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
    let object_format = git_object_algorithm(&commit)?;
    let pending = PendingResolvedGitSource {
        requested_locator: requested_locator.to_owned(),
        lineage: lineage.clone(),
        locator_identity: locator_identity.to_owned(),
        transport_profile: execution_transport.profile(),
        requested_rev: requested_rev.to_owned(),
        object_format,
        commit,
        materialized_tree: materialized.materialized_tree,
        tree,
        snapshot_root: materialized.snapshot_root,
        local: materialized.local,
        workspace_projection: materialized.workspace_projection,
        source_limits: limits,
    };
    Ok((pending, materialized.evidence))
}
