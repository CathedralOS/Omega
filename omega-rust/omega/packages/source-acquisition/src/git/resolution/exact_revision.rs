// Whole-repository Git resolution at an exact recorded commit and root-tree ID
// (`resolve_git_source_at_revision_in_lane`, `GitExactRevisionAcquisition`).
//
// `Offline` inspects verified cached objects only and preserves a healthy cache
// when the recorded commit or root tree is absent; `AllowFetch` probes cached
// objects first and performs one bounded fetch of the recorded commit on a
// typed absence before running full object authentication.

use crate::error::SourceResolveError;
use crate::git::executable::selection::PrimaryGitSelection;
use crate::git::request::GitSourceRequest;
use crate::identity::{GitCommitId, GitTreeId};
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::ResolvedGitSource;
use crate::storage::RetainedStorageLane;

use super::acquisition::resolve_whole_git_source_from_retained_lanes;
use super::selection::{GitRevisionSelection, RecordedGitRevision};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitExactRevisionAcquisition {
    Offline,
    AllowFetch,
}

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
