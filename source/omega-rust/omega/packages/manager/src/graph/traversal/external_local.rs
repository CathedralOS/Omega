//! Resolve explicitly selected local roots outside a workspace.

use super::super::reconciliation::{
    PackageRootSourceRequest, PackageSourceClosureLimits, ResolvedPackageSourceClosure,
};
use super::cache::{
    SourceCacheLane, resolve_external_local_package_from_cache,
    resolve_external_local_project_from_cache,
};
use super::dependency_resolution::resolve_registered_package_closure;
use super::errors::ResolveExternalLocalPackageClosureError;
use crate::source::identity::{ExternalSourceContext, PackageKey, SourceLineage};
use crate::source::package::ResolvePackageSourceError;
use crate::source::{LocalSourceLimits, SourceResolverStorage};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Resolve one explicitly selected non-workspace local package and its complete
/// Path/Git closure.
///
/// Every local package is snapshotted before its declaration or dependency rows
/// are consumed. Its lineage binds the canonical absolute path to the supplied
/// consuming context, so relative and absolute Path rows may cross directory
/// boundaries without pretending to be portable workspace dependencies. No
/// parent workspace or lock is discovered from the ambient filesystem.
#[cfg(test)]
pub(crate) fn resolve_external_local_package_closure(
    live_root: impl AsRef<Path>,
    source_context: ExternalSourceContext,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir).map_err(|error| {
        ResolveExternalLocalPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    resolve_external_local_package_closure_with_storage(
        live_root,
        source_context,
        &storage,
        source_limits,
        closure_limits,
    )
}

/// Resolve an external-local package closure beneath private resolver storage.
pub fn resolve_external_local_package_closure_with_storage(
    live_root: impl AsRef<Path>,
    source_context: ExternalSourceContext,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    resolve_external_local_declared_closure_with_storage(
        live_root.as_ref(),
        source_context,
        storage,
        source_limits,
        closure_limits,
        false,
    )
}

/// Resolve a local compilation root and its complete declared dependency
/// closure. The root may be an application or a package; every dependency is
/// still required to be a package.
pub fn resolve_external_local_project_closure_with_storage(
    live_root: impl AsRef<Path>,
    source_context: ExternalSourceContext,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    resolve_external_local_declared_closure_with_storage(
        live_root.as_ref(),
        source_context,
        storage,
        source_limits,
        closure_limits,
        true,
    )
}

fn resolve_external_local_declared_closure_with_storage(
    live_root: &Path,
    source_context: ExternalSourceContext,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    application_root_allowed: bool,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    storage.verify_path_identity().map_err(|error| {
        ResolveExternalLocalPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    let result = resolve_external_local_declared_closure_from_lanes(
        live_root,
        source_context,
        SourceCacheLane::Retained(storage.external_local_sources()),
        SourceCacheLane::Retained(storage.workspace_members()),
        SourceCacheLane::Retained(storage.git_sources()),
        source_limits,
        closure_limits,
        application_root_allowed,
    );
    storage.verify_path_identity().map_err(|error| {
        ResolveExternalLocalPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    result
}

#[allow(clippy::too_many_arguments)]
fn resolve_external_local_declared_closure_from_lanes(
    live_root: &Path,
    source_context: ExternalSourceContext,
    local_cache: SourceCacheLane<'_>,
    workspace_cache: SourceCacheLane<'_>,
    git_cache: SourceCacheLane<'_>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    application_root_allowed: bool,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    let requested_root = live_root.to_path_buf();
    let root_request = PackageRootSourceRequest::ExternalLocal {
        requested_root: requested_root.clone(),
        source_context: source_context.clone(),
    };
    let root = if application_root_allowed {
        resolve_external_local_project_from_cache(
            &requested_root,
            local_cache,
            source_limits,
            source_context.clone(),
        )
    } else {
        resolve_external_local_package_from_cache(
            &requested_root,
            local_cache,
            source_limits,
            source_context.clone(),
        )
    }
    .map_err(ResolveExternalLocalPackageClosureError::Root)?;
    if root.source().requested_root() != requested_root
        || !matches!(
            root.key().source_lineage(),
            SourceLineage::ExternalLocal(lineage)
                if lineage.source_context() == &source_context
        )
    {
        return Err(ResolveExternalLocalPackageClosureError::RootRequestMismatch);
    }
    let mut external_roots: BTreeMap<PackageKey, PathBuf> = BTreeMap::from([(
        root.key().clone(),
        root.source().canonical_live_root().to_path_buf(),
    )]);

    resolve_registered_package_closure(
        root_request,
        root.into_custody(),
        closure_limits,
        workspace_cache,
        git_cache,
        local_cache,
        source_limits,
        &mut BTreeMap::new(),
        &mut external_roots,
        Some(&source_context),
    )
    .map_err(ResolveExternalLocalPackageClosureError::Closure)
}
