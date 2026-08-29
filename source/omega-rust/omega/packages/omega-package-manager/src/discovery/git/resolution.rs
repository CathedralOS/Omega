//! Resolve Git source requests and select the exact package snapshot to bind.

use super::binding::{bind_git_root_package_source, bind_projected_git_package_source};
use super::request::GitPackageSourceRequest;
use super::workspace::{
    MAX_BUILD_DECLARATION_BYTES, MAX_TOTAL_BUILD_DECLARATION_BYTES, MAX_WORKSPACE_MEMBERS,
    ManagerGitWorkspacePlanner,
};
use crate::declarations::dependencies::read::PackageSelection;
use crate::discovery::{ResolvePackageSourceError, ResolvedPackageSource};
use omega_package_source::git::resolution::{
    resolve_git_source_in_lane, resolve_git_workspace_member_from_pin_in_lanes,
};
use omega_package_source::storage::RetainedStorageLane;
use omega_package_source::{
    GitAcquisitionPin, GitSourceRequest, GitWorkspaceDeclarationLimits,
    GitWorkspaceProjectionError, LocalSourceLimits, ResolvedGitSource, SourceResolverStorage,
};

#[cfg(test)]
pub fn resolve_git_package_source(
    request: &GitSourceRequest,
    cache_dir: impl AsRef<std::path::Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir)?;
    resolve_git_package_source_with_storage(request, &storage, limits)
}

fn resolve_git_root_package_source_in_lane(
    request: &GitSourceRequest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    application_root_allowed: bool,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let limits = limits.compiler_bounded();
    let lineage = request.lineage().clone();
    let source = resolve_git_source_in_lane(request, lane, limits)?;
    bind_git_root_package_source(lineage, source, limits, application_root_allowed)
}

fn resolve_selected_git_package_source_in_lanes(
    request: &GitPackageSourceRequest,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    resolve_selected_git_package_source_from_pin_in_lanes(
        request,
        None,
        git_lane,
        member_lane,
        limits,
    )
}

pub(crate) fn resolve_selected_git_package_source_from_pin_in_lanes(
    request: &GitPackageSourceRequest,
    pin: Option<&GitAcquisitionPin>,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    resolve_selected_git_declared_source_from_pin_in_lanes(
        request,
        pin,
        git_lane,
        member_lane,
        limits,
        false,
    )
}

pub(crate) fn resolve_selected_git_project_source_from_pin_in_lanes(
    request: &GitPackageSourceRequest,
    pin: Option<&GitAcquisitionPin>,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    resolve_selected_git_declared_source_from_pin_in_lanes(
        request,
        pin,
        git_lane,
        member_lane,
        limits,
        true,
    )
}

fn resolve_selected_git_declared_source_from_pin_in_lanes(
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
            request.acquisition(),
            git_lane,
            limits,
            application_root_allowed,
        ),
        PackageSelection::Named(package) => {
            let mut planner = ManagerGitWorkspacePlanner::new(package);
            let projected = resolve_git_workspace_member_from_pin_in_lanes(
                request.acquisition(),
                pin,
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
            bind_projected_git_package_source(
                request.acquisition().lineage().clone(),
                source,
                limits,
                evidence,
                application_root_allowed,
            )
        }
    }
}

pub fn resolve_git_package_source_with_storage(
    request: &GitSourceRequest,
    storage: &SourceResolverStorage,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    resolve_selected_git_package_source_with_storage(
        &GitPackageSourceRequest::root(request.clone()),
        storage,
        limits,
    )
}

pub fn resolve_selected_git_package_source_with_storage(
    request: &GitPackageSourceRequest,
    storage: &SourceResolverStorage,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    storage.verify_path_identity()?;
    let result = resolve_selected_git_package_source_in_lanes(
        request,
        storage.git_sources(),
        storage.workspace_members(),
        limits,
    );
    storage.verify_path_identity()?;
    result
}

pub fn resolve_selected_git_project_source_with_storage(
    request: &GitPackageSourceRequest,
    storage: &SourceResolverStorage,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    storage.verify_path_identity()?;
    let result = resolve_selected_git_project_source_from_pin_in_lanes(
        request,
        None,
        storage.git_sources(),
        storage.workspace_members(),
        limits,
    );
    storage.verify_path_identity()?;
    result
}
