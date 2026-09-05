//! Whole-root acquisition of inert recorded object IDs, without selector drift.

use crate::error::SourceResolveError;
use crate::git::executable::selection::{PrimaryGitSelection, resolver_package_controlled_roots};
use crate::git::request::GitSourceRequest;
use crate::git::workspace::GitWorkspaceProjectionError;
use crate::identity::{GitCommitId, GitTreeId};
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::ResolvedGitSource;
use crate::storage::RetainedStorageLane;

use super::acquisition::resolve_git_source_from_retained_cache_with_selection;
use super::materialization::materialize_whole_git_source;
use super::selection::{GitRevisionSelection, RecordedGitRevision};

/// Explicit permission to acquire exactly the recorded revision, never a ref.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitExactRevisionAcquisition {
    /// Inspect verified cached objects only. Absence of the recorded commit or
    /// root tree preserves a healthy cache and never initiates transport or discovery.
    Offline,
    /// Reuse exact cached objects first; a typed absence permits one bounded
    /// fetch of the recorded commit, followed by ordinary object verification.
    AllowFetch,
}

/// Reconstruct whole-source custody from recorded commit/root-tree identities.
/// The original authored request remains in the result and cache metadata.
/// IDs are expected content, not proof of acquisition or package acceptance.
/// Named workspace member selection is not performed by this entrance.
/// Missing or corrupt descendant objects fail normal source authentication;
/// this entrance does not automatically repair an incomplete descendant graph.
pub fn resolve_git_source_at_revision_in_lane(
    request: &GitSourceRequest,
    commit: &GitCommitId,
    tree: &GitTreeId,
    acquisition: GitExactRevisionAcquisition,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    let recorded = RecordedGitRevision::new(request, commit, tree, acquisition)?;
    resolve_recorded(lane.primary_git()?, request, &recorded, lane, limits)
}

/// Exact recorded revision acquisition using an explicit operator-selected Git.
pub fn resolve_git_source_at_revision_in_lane_with_primary_git(
    primary_git: &PrimaryGitSelection,
    request: &GitSourceRequest,
    commit: &GitCommitId,
    tree: &GitTreeId,
    acquisition: GitExactRevisionAcquisition,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    let recorded = RecordedGitRevision::new(request, commit, tree, acquisition)?;
    resolve_recorded(primary_git, request, &recorded, lane, limits)
}

fn resolve_recorded(
    primary_git: &PrimaryGitSelection,
    request: &GitSourceRequest,
    recorded: &RecordedGitRevision,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    let package_controlled_roots = resolver_package_controlled_roots(&[lane.path()])?;
    lane.verify_path_identity()?;
    let result = resolve_git_source_from_retained_cache_with_selection(
        primary_git,
        &package_controlled_roots,
        request,
        lane.path(),
        lane.directory(),
        limits.compiler_bounded(),
        GitRevisionSelection::Recorded(recorded),
        materialize_whole_git_source,
    );
    lane.verify_path_identity()?;
    match result {
        Ok((source, ())) => Ok(source),
        Err(GitWorkspaceProjectionError::Source(error)) => Err(error),
        Err(GitWorkspaceProjectionError::Planner(never)) => match never {},
    }
}
