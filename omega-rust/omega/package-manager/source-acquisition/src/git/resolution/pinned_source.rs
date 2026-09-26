//! Whole-source reuse of a pin issued earlier in the same acquisition operation.

use super::resolve_git_source_in_lane_with_selected_roots;
use crate::error::SourceResolveError;
use crate::git::executable::selection::{PrimaryGitSelection, resolver_package_controlled_roots};
use crate::git::request::GitSourceRequest;
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::{GitAcquisitionPin, ResolvedGitSource};
use crate::storage::RetainedStorageLane;

/// Reuse an operation-local acquisition for its whole repository root with the
/// selected primary Git (`lane.primary_git()?` trusts the lane's own).
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
    let package_controlled_roots = resolver_package_controlled_roots(&[lane.path()])?;
    resolve_git_source_in_lane_with_selected_roots(
        primary_git,
        &package_controlled_roots,
        request,
        pin,
        lane,
        limits,
    )
}
