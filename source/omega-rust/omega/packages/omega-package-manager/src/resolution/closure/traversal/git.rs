//! Resolve immutable Git package roots.

use super::super::reconciliation::{
    PackageRootSourceRequest, PackageSourceClosureLimits, ResolvedPackageSourceClosure,
};
use super::cache::{SourceCacheLane, resolve_git_from_cache};
use super::dependency_resolution::{register_workspace, resolve_registered_package_closure};
use super::errors::ResolveGitPackageClosureError;
use crate::resolution::binding::ResolvePackageSourceError;
use omega_package_source::SourceLineage;
use omega_package_source::{
    GitSourceRequest, LocalSourceLimits, ResolvedGitSource, SourceResolverStorage,
};
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
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveGitPackageClosureError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir).map_err(|error| {
        ResolveGitPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    resolve_git_package_closure_with_storage(request, &storage, source_limits, closure_limits)
}

fn resolve_git_package_closure_from_lanes(
    request: &GitSourceRequest,
    workspace_cache: SourceCacheLane<'_>,
    git_cache: SourceCacheLane<'_>,
    local_cache: SourceCacheLane<'_>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveGitPackageClosureError> {
    let root = resolve_git_from_cache(request, git_cache, source_limits)
        .map_err(ResolveGitPackageClosureError::Root)?;
    if !git_root_request_matches(request, root.source(), root.key().source_lineage()) {
        return Err(ResolveGitPackageClosureError::RootRequestMismatch);
    }
    let mut workspaces = BTreeMap::new();
    register_workspace(
        &mut workspaces,
        root.key().source_lineage(),
        root.snapshot_root(),
    )
    .map_err(ResolveGitPackageClosureError::RootWorkspace)?;

    resolve_registered_package_closure(
        PackageRootSourceRequest::Git(request.clone()),
        root.into_custody(),
        closure_limits,
        workspace_cache,
        git_cache,
        local_cache,
        source_limits,
        &mut workspaces,
        &mut BTreeMap::new(),
        None,
    )
    .map_err(ResolveGitPackageClosureError::Closure)
}

/// Resolve a Git closure beneath the manager-owned private source root.
pub fn resolve_git_package_closure_with_storage(
    request: &GitSourceRequest,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveGitPackageClosureError> {
    storage.verify_path_identity().map_err(|error| {
        ResolveGitPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    let result = resolve_git_package_closure_from_lanes(
        request,
        SourceCacheLane::Retained(storage.workspace_members()),
        SourceCacheLane::Retained(storage.git_sources()),
        SourceCacheLane::Retained(storage.external_local_sources()),
        source_limits,
        closure_limits,
    );
    storage.verify_path_identity().map_err(|error| {
        ResolveGitPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    result
}

pub(crate) fn git_root_request_matches(
    request: &GitSourceRequest,
    resolved: &ResolvedGitSource,
    lineage: &SourceLineage,
) -> bool {
    resolved.requested_locator() == request.requested_locator()
        && resolved.locator_identity() == request.locator_identity()
        && resolved.requested_revision() == request.requested_revision()
        && resolved.transport_profile() == request.transport_profile()
        && lineage == request.lineage()
}
