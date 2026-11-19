// Exact recorded-revision workspace member resolution
// (`resolve_git_workspace_member_at_revision_in_lanes`).
//
// After authenticating the commit and tree graph for the recorded `(commit,
// tree)` pair, opens only the root/member declarations and the selected
// member's payload closure, and materializes only the selected member.

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
