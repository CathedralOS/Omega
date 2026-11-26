//! Resolve a full commit, optionally checking its tree against an independent record.
//! Online resolution fetches the commit when missing. Offline resolution uses retained objects.

use crate::error::SourceResolveError;
use crate::git::executable::selection::PrimaryGitSelection;
use crate::git::request::GitSourceRequest;
use crate::identity::{GitCommitId, GitTreeId};
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::ResolvedGitSource;
use crate::storage::RetainedStorageLane;

use super::acquisition::resolve_whole_git_source_from_retained_lanes;
use super::selection::{GitRevisionSelection, ExactGitRevision};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitExactRevisionAcquisition {
    Offline,
    AllowFetch,
}

pub fn resolve_git_source_at_revision_in_lane(
    primary_git: &PrimaryGitSelection,
    request: &GitSourceRequest,
    commit: &GitCommitId,
    tree: Option<&GitTreeId>,
    acquisition: GitExactRevisionAcquisition,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    let recorded = ExactGitRevision::new(request, commit, tree, acquisition)?;
    resolve_whole_git_source_from_retained_lanes(
        primary_git,
        &[lane],
        request,
        limits,
        GitRevisionSelection::Exact(&recorded),
    )
}
