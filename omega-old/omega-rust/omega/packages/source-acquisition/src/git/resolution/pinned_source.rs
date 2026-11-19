// Whole-repository Git resolution reusing an operation-local `GitAcquisitionPin`
// (`resolve_git_source_from_pin_in_lane`).
//
// When `pin` is `Some`, it must match the exact request and retained commit/root
// tree in the cache without fetching; `None` performs ordinary resolution.

use super::acquisition::resolve_whole_git_source_from_retained_lanes;
use super::selection::GitRevisionSelection;
use crate::error::SourceResolveError;
use crate::git::executable::selection::PrimaryGitSelection;
use crate::git::request::GitSourceRequest;
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::{GitAcquisitionPin, ResolvedGitSource};
use crate::storage::RetainedStorageLane;

pub fn resolve_git_source_from_pin_in_lane(
    primary_git: &PrimaryGitSelection,
    request: &GitSourceRequest,
    pin: Option<&GitAcquisitionPin>,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    resolve_whole_git_source_from_retained_lanes(
        primary_git,
        &[lane],
        request,
        limits,
        GitRevisionSelection::Ordinary(pin),
    )
}
