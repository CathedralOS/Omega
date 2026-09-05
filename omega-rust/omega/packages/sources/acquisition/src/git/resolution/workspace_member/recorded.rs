//! Exact recorded repository selection with selective workspace materialization.

use crate::git::executable::selection::{PrimaryGitSelection, resolver_package_controlled_roots};
use crate::git::request::GitSourceRequest;
use crate::git::workspace::{
    GitWorkspaceDeclarationLimits, GitWorkspaceProjectionError, GitWorkspaceProjectionPlanner,
    GitWorkspaceProjectionResult,
};
use crate::identity::{GitCommitId, GitTreeId};
use crate::limits::LocalSourceLimits;
use crate::storage::RetainedStorageLane;

use super::super::exact_revision::GitExactRevisionAcquisition;
use super::super::selection::{GitRevisionSelection, RecordedGitRevision};
use super::resolve_git_workspace_member_with_selected_roots;

/// Select one declared member from the exact recorded repository revision.
/// After authenticated tree-graph inspection, only declaration and selected
/// member payloads are opened, and only the member is materialized.
/// The planner, not source acquisition, owns package syntax.
/// The original request remains retained; recorded IDs do not mint custody.
pub fn resolve_git_workspace_member_at_revision_in_lanes<Planner>(
    request: &GitSourceRequest,
    commit: &GitCommitId,
    tree: &GitTreeId,
    acquisition: GitExactRevisionAcquisition,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    declaration_limits: GitWorkspaceDeclarationLimits,
    planner: &mut Planner,
) -> Result<
    GitWorkspaceProjectionResult<Planner::Evidence>,
    GitWorkspaceProjectionError<Planner::Error>,
>
where
    Planner: GitWorkspaceProjectionPlanner,
{
    let recorded = RecordedGitRevision::new(request, commit, tree, acquisition)?;
    resolve_recorded_member(
        git_lane.primary_git()?,
        request,
        &recorded,
        git_lane,
        member_lane,
        limits,
        declaration_limits,
        planner,
    )
}

/// The same exact member selection with an explicit operator-selected Git.
pub fn resolve_git_workspace_member_at_revision_in_lanes_with_primary_git<Planner>(
    primary_git: &PrimaryGitSelection,
    request: &GitSourceRequest,
    commit: &GitCommitId,
    tree: &GitTreeId,
    acquisition: GitExactRevisionAcquisition,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    declaration_limits: GitWorkspaceDeclarationLimits,
    planner: &mut Planner,
) -> Result<
    GitWorkspaceProjectionResult<Planner::Evidence>,
    GitWorkspaceProjectionError<Planner::Error>,
>
where
    Planner: GitWorkspaceProjectionPlanner,
{
    let recorded = RecordedGitRevision::new(request, commit, tree, acquisition)?;
    resolve_recorded_member(
        primary_git,
        request,
        &recorded,
        git_lane,
        member_lane,
        limits,
        declaration_limits,
        planner,
    )
}

fn resolve_recorded_member<Planner>(
    primary_git: &PrimaryGitSelection,
    request: &GitSourceRequest,
    recorded: &RecordedGitRevision,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    declaration_limits: GitWorkspaceDeclarationLimits,
    planner: &mut Planner,
) -> Result<
    GitWorkspaceProjectionResult<Planner::Evidence>,
    GitWorkspaceProjectionError<Planner::Error>,
>
where
    Planner: GitWorkspaceProjectionPlanner,
{
    let package_controlled_roots =
        resolver_package_controlled_roots(&[git_lane.path(), member_lane.path()])?;
    resolve_git_workspace_member_with_selected_roots(
        primary_git,
        &package_controlled_roots,
        request,
        GitRevisionSelection::Recorded(recorded),
        git_lane,
        member_lane,
        limits.compiler_bounded(),
        declaration_limits,
        planner,
    )
}
