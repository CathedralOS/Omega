//! Resolve immutable Git package roots.

use super::super::reconcile::{
    PackageRootSourceRequest, PackageSourceClosureLimits, ResolvedPackageSourceClosure,
};
use super::cache::{GitAcquisitionCache, SourceCacheLane};
use super::dependencies::{register_git_repository, resolve_registered_package_closure};
use super::errors::ResolveGitPackageClosureError;
use crate::resolution::source::{
    GitPackageSourceRequest, PackageSourceNavigation, ResolvePackageSourceError,
};
use omega_package_source::{
    GitSourceRequest, LocalSourceLimits, ResolvedGitSource, SourceResolverStorage,
};
use omega_target::TargetProfile;
use std::collections::BTreeMap;
#[cfg(test)]
use std::path::Path;

/// Resolve one repository-root Git package and its complete Path/Git closure.
///
/// The exact validated root request is retained independently from normalized
/// lineage and immutable commit/tree/content identity. A Git root that is a
/// multi-package workspace remains ambiguous until the explicit package
/// selector design is implemented.
#[cfg(test)]
pub(crate) fn resolve_git_package_closure(
    request: &GitSourceRequest,
    target_profile: TargetProfile,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveGitPackageClosureError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir).map_err(|error| {
        ResolveGitPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    resolve_git_package_closure_with_storage(
        request,
        target_profile,
        &storage,
        source_limits,
        closure_limits,
    )
}

fn resolve_git_package_closure_from_lanes(
    request: &GitPackageSourceRequest,
    target_profile: TargetProfile,
    workspace_cache: SourceCacheLane<'_>,
    git_cache: SourceCacheLane<'_>,
    local_cache: SourceCacheLane<'_>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    application_root_allowed: bool,
) -> Result<ResolvedPackageSourceClosure, ResolveGitPackageClosureError> {
    let mut git_acquisitions = GitAcquisitionCache::default();
    let root = if application_root_allowed {
        git_acquisitions.resolve_selected_project(
            request,
            git_cache,
            workspace_cache,
            source_limits,
        )
    } else {
        git_acquisitions.resolve_selected(request, git_cache, workspace_cache, source_limits)
    }
    .map_err(ResolveGitPackageClosureError::Root)?;
    if !git_package_root_request_matches(request, &root) {
        return Err(ResolveGitPackageClosureError::RootRequestMismatch);
    }
    let mut workspaces = BTreeMap::new();
    register_git_repository(
        &mut workspaces,
        request.acquisition(),
        root.key().source_lineage(),
        root.resolution(),
        root.selection_evidence(),
        root.source_limits(),
    )
    .map_err(ResolveGitPackageClosureError::RootWorkspace)?;

    resolve_registered_package_closure(
        PackageRootSourceRequest::Git(request.clone()),
        root.into_custody(),
        target_profile,
        closure_limits,
        workspace_cache,
        git_cache,
        local_cache,
        source_limits,
        &mut workspaces,
        &mut BTreeMap::new(),
        None,
        &mut git_acquisitions,
    )
    .map_err(ResolveGitPackageClosureError::Closure)
}

/// Resolve a Git closure beneath the manager-owned private source root.
pub fn resolve_git_package_closure_with_storage(
    request: &GitSourceRequest,
    target_profile: TargetProfile,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveGitPackageClosureError> {
    resolve_selected_git_package_closure_with_storage(
        &GitPackageSourceRequest::root(request.clone()),
        target_profile,
        storage,
        source_limits,
        closure_limits,
    )
}

/// Resolve a repository-root Git project. The selected root may be a package
/// or application; every dependency remains package-only.
pub fn resolve_git_project_closure_with_storage(
    request: &GitSourceRequest,
    target_profile: TargetProfile,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveGitPackageClosureError> {
    resolve_selected_git_project_closure_with_storage(
        &GitPackageSourceRequest::root(request.clone()),
        target_profile,
        storage,
        source_limits,
        closure_limits,
    )
}

/// Resolve one explicitly selected package from a Git repository and its closure.
pub fn resolve_selected_git_package_closure_with_storage(
    request: &GitPackageSourceRequest,
    target_profile: TargetProfile,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveGitPackageClosureError> {
    storage.verify_path_identity().map_err(|error| {
        ResolveGitPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    let result = resolve_git_package_closure_from_lanes(
        request,
        target_profile,
        SourceCacheLane::Retained(storage.workspace_members()),
        SourceCacheLane::Retained(storage.git_sources()),
        SourceCacheLane::Retained(storage.external_local_sources()),
        source_limits,
        closure_limits,
        false,
    );
    storage.verify_path_identity().map_err(|error| {
        ResolveGitPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    result
}

/// Resolve one explicitly selected Git project root. Named workspace members
/// may be packages or applications at this root boundary.
pub fn resolve_selected_git_project_closure_with_storage(
    request: &GitPackageSourceRequest,
    target_profile: TargetProfile,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveGitPackageClosureError> {
    storage.verify_path_identity().map_err(|error| {
        ResolveGitPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    let result = resolve_git_package_closure_from_lanes(
        request,
        target_profile,
        SourceCacheLane::Retained(storage.workspace_members()),
        SourceCacheLane::Retained(storage.git_sources()),
        SourceCacheLane::Retained(storage.external_local_sources()),
        source_limits,
        closure_limits,
        true,
    );
    storage.verify_path_identity().map_err(|error| {
        ResolveGitPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    result
}

pub(crate) fn git_root_request_matches(
    request: &GitSourceRequest,
    resolved: &ResolvedGitSource,
) -> bool {
    resolved.requested_locator() == request.requested_locator()
        && resolved.locator_identity() == request.locator_identity()
        && resolved.requested_revision() == request.requested_revision()
        && resolved.transport_profile() == request.transport_profile()
        && resolved.lineage() == request.lineage()
}

fn git_package_root_request_matches(
    request: &GitPackageSourceRequest,
    resolved: &crate::resolution::source::ResolvedPackageSource<ResolvedGitSource>,
) -> bool {
    if !git_root_request_matches(request.acquisition(), resolved.source())
        || resolved.key().source_lineage() != resolved.source().lineage()
    {
        return false;
    }
    match (request.selection(), resolved.navigation()) {
        (
            crate::declarations::dependencies::read::PackageSelection::Root,
            PackageSourceNavigation::Root,
        ) => true,
        (
            crate::declarations::dependencies::read::PackageSelection::Named(package),
            PackageSourceNavigation::Member(_),
        ) => resolved.key().name() == package,
        _ => false,
    }
}
