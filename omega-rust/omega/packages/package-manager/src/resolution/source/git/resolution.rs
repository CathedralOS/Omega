//! Resolve Git source requests and select the exact package snapshot to bind.

use super::binding::{bind_git_root_package_source, bind_git_workspace_member_source};
use super::request::GitPackageSourceRequest;
use super::workspace::{ManagerGitWorkspacePlanner, git_workspace_declaration_limits};
use crate::declarations::dependencies::read::PackageSelection;
use crate::resolution::source::{ResolvePackageSourceError, ResolvedPackageSource};
use package_source::git::resolution::{
    resolve_git_source_from_pin_in_lane, resolve_git_workspace_member_from_pin_in_lanes,
};
use package_source::storage::RetainedStorageLane;
use package_source::{
    GitAcquisitionPin, GitSourceRequest, LocalSourceLimits, ResolvedGitSource, SourceResolver,
};

fn resolve_git_root_package_source_in_lane(
    resolver: &SourceResolver,
    request: &GitSourceRequest,
    pin: Option<&GitAcquisitionPin>,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    application_root_allowed: bool,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let limits = limits.compiler_bounded();
    let source =
        resolve_git_source_from_pin_in_lane(resolver.primary_git()?, request, pin, lane, limits)?;
    bind_git_root_package_source(source, application_root_allowed)
}
pub(crate) fn resolve_selected_git_package_source_from_pin_in_lanes(
    resolver: &SourceResolver,
    request: &GitPackageSourceRequest,
    pin: Option<&GitAcquisitionPin>,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    resolve_selected_git_declared_source_from_pin_in_lanes(
        resolver,
        request,
        pin,
        git_lane,
        member_lane,
        limits,
        false,
    )
}

pub(crate) fn resolve_selected_git_project_source_from_pin_in_lanes(
    resolver: &SourceResolver,
    request: &GitPackageSourceRequest,
    pin: Option<&GitAcquisitionPin>,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    resolve_selected_git_declared_source_from_pin_in_lanes(
        resolver,
        request,
        pin,
        git_lane,
        member_lane,
        limits,
        true,
    )
}

fn resolve_selected_git_declared_source_from_pin_in_lanes(
    resolver: &SourceResolver,
    request: &GitPackageSourceRequest,
    pin: Option<&GitAcquisitionPin>,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    application_root_allowed: bool,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let limits = limits.compiler_bounded();
    match request.selection() {
        PackageSelection::Root => resolve_git_root_package_source_in_lane(
            resolver,
            request.acquisition(),
            pin,
            git_lane,
            limits,
            application_root_allowed,
        ),
        PackageSelection::Named(package) => {
            let mut planner = ManagerGitWorkspacePlanner::new(package);
            bind_git_workspace_member_source(
                resolve_git_workspace_member_from_pin_in_lanes(
                    resolver.primary_git()?,
                    request.acquisition(),
                    pin,
                    git_lane,
                    member_lane,
                    limits,
                    git_workspace_declaration_limits(),
                    &mut planner,
                ),
                application_root_allowed,
            )
        }
    }
}
pub(crate) fn resolve_selected_git_project_source(
    request: &GitPackageSourceRequest,
    storage: &SourceResolver,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    storage.verify_path_identity()?;
    let result = resolve_selected_git_project_source_from_pin_in_lanes(
        storage,
        request,
        None,
        storage.git_sources(),
        storage.workspace_members(),
        limits,
    );
    storage.verify_path_identity()?;
    result
}
