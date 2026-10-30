//! Whole-root acquisition of inert recorded object IDs, without selector drift.

use crate::error::SourceResolveError;
use crate::git::executable::selection::PrimaryGitSelection;
use crate::git::request::GitSourceRequest;
use crate::identity::{GitCommitId, GitTreeId};
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::ResolvedGitSource;
use crate::storage::RetainedStorageLane;

use super::acquisition::resolve_whole_git_source_from_retained_lanes;
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

/// Reconstruct whole-source custody from recorded commit/root-tree identities
/// with the selected primary Git from `resolver.primary_git()?`.
/// The original authored request remains in the result and cache metadata.
/// IDs are expected content, not proof of acquisition or package acceptance.
/// Named workspace member selection is not performed by this entrance.
/// Missing or corrupt descendant objects fail normal source authentication;
/// this entrance does not automatically repair an incomplete descendant graph.
pub fn resolve_git_source_at_revision_in_lane(
    primary_git: &PrimaryGitSelection,
    request: &GitSourceRequest,
    commit: &GitCommitId,
    tree: &GitTreeId,
    acquisition: GitExactRevisionAcquisition,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    let recorded = RecordedGitRevision::new(request, commit, tree, acquisition)?;
    resolve_whole_git_source_from_retained_lanes(
        primary_git,
        &[lane],
        request,
        limits,
        GitRevisionSelection::Recorded(&recorded),
    )
}
