//! Whole-source reuse of a pin issued earlier in the same acquisition operation.

use super::acquisition::resolve_whole_git_source_from_retained_lanes;
use super::selection::GitRevisionSelection;
use crate::error::SourceResolveError;
use crate::git::executable::selection::PrimaryGitSelection;
use crate::git::request::GitSourceRequest;
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::{GitAcquisitionPin, ResolvedGitSource};
use crate::storage::RetainedStorageLane;

/// Reuse an operation-local acquisition for its whole repository root with the
/// selected primary Git from `resolver.primary_git()?`.
/// A supplied pin must match the exact request and retained commit/root tree;
/// missing or invalid cache state fails without fetching. `None` has ordinary
/// source-resolution behavior. This does not recover a persisted lock pin.
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
