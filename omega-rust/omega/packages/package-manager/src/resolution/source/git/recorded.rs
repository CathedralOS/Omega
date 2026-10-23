//! Bind freshly acquired exact recorded Git content using ordinary declarations.

use super::binding::{bind_git_root_package_source, bind_git_workspace_member_source};
use super::request::GitPackageSourceRequest;
use super::workspace::{ManagerGitWorkspacePlanner, git_workspace_declaration_limits};
use crate::declarations::dependencies::read::PackageSelection;
use crate::resolution::source::{ResolvePackageSourceError, ResolvedPackageSource, SourceRootRole};
use package_source::git::resolution::{
    GitExactRevisionAcquisition, resolve_git_source_at_revision_in_lane,
    resolve_git_workspace_member_at_revision_in_lanes,
};
use package_source::storage::RetainedStorageLane;
use package_source::{
    GitCommitId, GitTreeId, LocalSourceLimits, ResolvedGitSource, SourceResolver,
};

pub(crate) fn resolve_selected_git_source_at_revision_in_lanes(
    resolver: &SourceResolver,
    request: &GitPackageSourceRequest,
    commit: &GitCommitId,
    tree: &GitTreeId,
    acquisition: GitExactRevisionAcquisition,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    role: SourceRootRole,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let limits = limits.compiler_bounded();
    match request.selection() {
        PackageSelection::Root => {
            let source = resolve_git_source_at_revision_in_lane(
                resolver.primary_git()?,
                request.acquisition(),
                commit,
                tree,
                acquisition,
                git_lane,
                limits,
            )?;
            bind_git_root_package_source(source, role)
        }
        PackageSelection::Named(package) => {
            let mut planner = ManagerGitWorkspacePlanner::new(package);
            bind_git_workspace_member_source(
                resolve_git_workspace_member_at_revision_in_lanes(
                    resolver.primary_git()?,
                    request.acquisition(),
                    commit,
                    tree,
                    acquisition,
                    git_lane,
                    member_lane,
                    limits,
                    git_workspace_declaration_limits(),
                    &mut planner,
                ),
                role,
            )
        }
    }
}
