//! Bind freshly acquired exact recorded Git content using ordinary declarations.

use super::binding::{bind_git_root_package_source, bind_projected_git_package_source};
use super::request::GitPackageSourceRequest;
use super::workspace::{
    MAX_BUILD_DECLARATION_BYTES, MAX_TOTAL_BUILD_DECLARATION_BYTES, MAX_WORKSPACE_MEMBERS,
    ManagerGitWorkspacePlanner,
};
use crate::declarations::dependencies::read::PackageSelection;
use crate::resolution::source::{ResolvePackageSourceError, ResolvedPackageSource};
use package_source::git::resolution::{
    GitExactRevisionAcquisition, resolve_git_source_at_revision_in_lane,
    resolve_git_workspace_member_at_revision_in_lanes,
};
use package_source::storage::RetainedStorageLane;
use package_source::{
    GitCommitId, GitTreeId, GitWorkspaceDeclarationLimits, GitWorkspaceProjectionError,
    LocalSourceLimits, ResolvedGitSource,
};

pub(crate) fn resolve_selected_git_package_source_at_revision_in_lanes(
    request: &GitPackageSourceRequest,
    commit: &GitCommitId,
    tree: &GitTreeId,
    acquisition: GitExactRevisionAcquisition,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    resolve_selected_git_declared_source_at_revision_in_lanes(
        request,
        commit,
        tree,
        acquisition,
        git_lane,
        member_lane,
        limits,
        false,
    )
}

pub(crate) fn resolve_selected_git_project_source_at_revision_in_lanes(
    request: &GitPackageSourceRequest,
    commit: &GitCommitId,
    tree: &GitTreeId,
    acquisition: GitExactRevisionAcquisition,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    resolve_selected_git_declared_source_at_revision_in_lanes(
        request,
        commit,
        tree,
        acquisition,
        git_lane,
        member_lane,
        limits,
        true,
    )
}

fn resolve_selected_git_declared_source_at_revision_in_lanes(
    request: &GitPackageSourceRequest,
    commit: &GitCommitId,
    tree: &GitTreeId,
    acquisition: GitExactRevisionAcquisition,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    application_root_allowed: bool,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let limits = limits.compiler_bounded();
    match request.selection() {
        PackageSelection::Root => {
            let source = resolve_git_source_at_revision_in_lane(
                request.acquisition(),
                commit,
                tree,
                acquisition,
                git_lane,
                limits,
            )?;
            bind_git_root_package_source(source, application_root_allowed)
        }
        PackageSelection::Named(package) => {
            let mut planner = ManagerGitWorkspacePlanner::new(package);
            let projected = resolve_git_workspace_member_at_revision_in_lanes(
                request.acquisition(),
                commit,
                tree,
                acquisition,
                git_lane,
                member_lane,
                limits,
                GitWorkspaceDeclarationLimits::new(
                    MAX_WORKSPACE_MEMBERS,
                    u64::try_from(MAX_BUILD_DECLARATION_BYTES)
                        .expect("declaration limit fits canonical u64"),
                    u64::try_from(MAX_TOTAL_BUILD_DECLARATION_BYTES)
                        .expect("declaration aggregate limit fits canonical u64"),
                ),
                &mut planner,
            )
            .map_err(|error| match error {
                GitWorkspaceProjectionError::Source(error) => {
                    ResolvePackageSourceError::Source(error)
                }
                GitWorkspaceProjectionError::Planner(error) => {
                    ResolvePackageSourceError::GitWorkspaceSelection(error)
                }
            })?;
            let (source, evidence) = projected.into_parts();
            bind_projected_git_package_source(source, evidence, application_root_allowed)
        }
    }
}
