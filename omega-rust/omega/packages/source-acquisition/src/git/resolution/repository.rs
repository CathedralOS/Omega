//! Resolve and authenticate one exact commit and root tree in a verified cache entry.

use crate::error::SourceResolveError;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::executor::GitExecutor;
use crate::git::objects::authentication::{authenticate_git_commit, verify_exact_git_revision};
use crate::git::objects::identity::is_object_id;
use crate::git::objects::{ExactGitObjectAvailability, probe_exact_git_object};
use crate::git::request::GitExecutionTransport;
use crate::git::workspace::GitWorkspaceProjectionError;
use crate::limits::LocalSourceLimits;
use cap_std::fs::Dir as CapabilityDirectory;
use std::ffi::OsStr;
use std::path::Path;

use super::network::bounded_git_fetch_arguments;
use super::recorded_objects::recorded_revision_needs_fetch;
use super::selection::GitRevisionSelection;

#[allow(clippy::too_many_arguments)]
pub(super) fn inspect_verified_git_cache_entry<Evidence, PlannerError>(
    executor: &GitExecutor,
    cache_directory: &CapabilityDirectory,
    entry_name: &OsStr,
    entry_root: &Path,
    locator_identity: &str,
    fetch_locator: &str,
    requested_rev: &str,
    execution_transport: GitExecutionTransport,
    limits: LocalSourceLimits,
    fetch_remote: bool,
    selection: GitRevisionSelection<'_>,
    inspect: impl FnOnce(
        &GitExecutor,
        &VerifiedGitRepository,
        &str,
        &str,
        LocalSourceLimits,
    ) -> Result<Evidence, GitWorkspaceProjectionError<PlannerError>>,
) -> Result<Evidence, GitWorkspaceProjectionError<PlannerError>> {
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
        GitRevisionSelection::Ordinary(None) if !fetch_remote && is_object_id(requested_rev) => {
            // An earlier failed full-ID fetch may have left a healthy empty
            // repository. Cache existence alone does not establish this object.
            // Only the exact absence protocol permits fetching the requested ID.
            matches!(
                probe_exact_git_object(executor, &repository, requested_rev)?,
                ExactGitObjectAvailability::Missing
            )
        }
        GitRevisionSelection::Ordinary(_) => fetch_remote,
    };
    if fetch_remote {
        let canonical_config = repository.read_canonical_config()?;
        let fetch_revision = selection.fetch_revision(requested_rev);
        let arguments = bounded_git_fetch_arguments(fetch_locator, fetch_revision, limits);
        let fetched = repository.run_git(executor, arguments.iter());
        if let Err(error @ SourceResolveError::GitCleanupFailed { .. }) = fetched {
            return Err(error.into());
        }
        // Git may update its configuration even when transport fails. Restore
        // the pre-fetch canonical bytes before checking whether this cache is
        // reusable; a fetch error must not poison otherwise healthy old pins.
        repository.restore_canonical_config(&canonical_config)?;
        fetched?;
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
    let evidence = inspect(executor, &repository, &commit, &tree, limits)?;
    repository.verify_current(limits)?;
    Ok(evidence)
}
