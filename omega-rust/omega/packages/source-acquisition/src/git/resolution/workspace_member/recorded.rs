//! Exact recorded repository selection with selective workspace materialization.

use crate::git::executable::selection::PrimaryGitSelection;
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
use super::resolve_git_workspace_member_at_selection;

/// Select one declared member from the exact recorded repository revision with
/// the selected primary Git from `resolver.primary_git()?`.
/// After authenticated tree-graph inspection, only declaration and selected
/// member payloads are opened, and only the member is materialized.
/// The planner, not source acquisition, owns package syntax.
/// The original request remains retained; recorded IDs do not mint custody.
pub fn resolve_git_workspace_member_at_revision_in_lanes<Planner>(
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
    resolve_git_workspace_member_at_selection(
        primary_git,
        request,
        GitRevisionSelection::Recorded(&recorded),
        git_lane,
        member_lane,
        limits.compiler_bounded(),
        declaration_limits,
        planner,
    )
}
