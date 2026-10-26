//! Resolve explicitly selected local roots outside a workspace.

use super::super::reconcile::{
    PackageRootSourceRequest, PackageSourceClosureLimits, ResolvedPackageSourceClosure,
};
use super::cache::{GitAcquisitionCache, SourceCacheLane};
use super::dependencies::resolve_registered_package_closure;
use super::errors::ResolveExternalLocalPackageClosureError;
use super::git_pins::GitResolutionOptions;
use crate::declarations::PackageKey;
use crate::resolution::source::{
    PackageSourceCustody, ResolvePackageSourceError, capture_external_local_project_source,
};
use package_source::{ExternalSourceContext, SourceLineage};
use package_source::{LocalSourceLimits, SourceResolver};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Resolve a local compilation root and its complete declared dependency
/// closure. The root may be an application or a package; every dependency is
/// still required to be a package. The Git policy in `options` applies to
/// every dependency, including requests first discovered in transitive
/// sources; accepted pins must belong to this exact root request.
pub(crate) fn resolve_external_local_project_closure(
    live_root: impl AsRef<Path>,
    source_context: ExternalSourceContext,
    storage: &SourceResolver,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    options: GitResolutionOptions<'_>,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    verify_pin_root(live_root.as_ref(), &source_context, options)?;
    storage.verify_path_identity().map_err(|error| {
        ResolveExternalLocalPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    let result = resolve_external_local_project_closure_from_lanes(
        live_root.as_ref(),
        source_context,
        SourceCacheLane::Retained(storage.external_local_sources()),
        SourceCacheLane::Retained(storage.workspace_members()),
        SourceCacheLane::Retained(storage.git_sources()),
        source_limits,
        closure_limits,
        &mut GitAcquisitionCache::with_options(storage, options),
    );
    storage.verify_path_identity().map_err(|error| {
        ResolveExternalLocalPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    result
}

fn verify_pin_root(
    live_root: &Path,
    source_context: &ExternalSourceContext,
    options: GitResolutionOptions<'_>,
) -> Result<(), ResolveExternalLocalPackageClosureError> {
    use crate::resolution::graph::CanonicalRootSourceRequest;
    if let Some(pins) = options.pins
        && !matches!(pins.accepted().root().request(),
        CanonicalRootSourceRequest::ExternalLocal { requested_root, source_context: context }
            if requested_root == live_root.as_os_str().as_encoded_bytes()
                && context == source_context)
    {
        return Err(ResolveExternalLocalPackageClosureError::RootRequestMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn resolve_external_local_project_closure_from_lanes(
    live_root: &Path,
    source_context: ExternalSourceContext,
    local_cache: SourceCacheLane<'_>,
    workspace_cache: SourceCacheLane<'_>,
    git_cache: SourceCacheLane<'_>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    git_acquisitions: &mut GitAcquisitionCache<'_>,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    let requested_root = live_root.to_path_buf();
    // A compilation root is captured fresh: this invocation reads the live
    // tree once here, and the captured snapshot owns those bytes afterwards.
    // A dependency source would resolve from the retained lane instead; that
    // mode belongs to `resolve_external_local_package_from_cache` alone.
    let root = capture_external_local_project_source(
        &requested_root,
        source_limits,
        source_context.clone(),
    )
    .map_err(ResolveExternalLocalPackageClosureError::Root)?;
    let canonical_live_root = root.snapshot_root().to_path_buf();
    if !matches!(
        root.key().source_lineage(),
        SourceLineage::ExternalLocal(lineage)
            if lineage.source_context() == &source_context
    ) {
        return Err(ResolveExternalLocalPackageClosureError::RootRequestMismatch);
    }
    let cache_dir = local_cache
        .checked_source_cache_dir()
        .map_err(ResolveExternalLocalPackageClosureError::Root)?;
    let review_cache_dir = local_cache
        .compiled_review_cache_dir()
        .map_err(ResolveExternalLocalPackageClosureError::Root)?;
    resolve_bound_external_local_closure(
        &requested_root,
        &canonical_live_root,
        source_context,
        root.with_checked_source_cache_dir(cache_dir)
            .with_compiled_review_cache_dir(review_cache_dir),
        local_cache,
        workspace_cache,
        git_cache,
        source_limits,
        closure_limits,
        git_acquisitions,
    )
}

#[allow(clippy::too_many_arguments)]
fn resolve_bound_external_local_closure(
    requested_root: &Path,
    canonical_live_root: &Path,
    source_context: ExternalSourceContext,
    root: PackageSourceCustody,
    local_cache: SourceCacheLane<'_>,
    workspace_cache: SourceCacheLane<'_>,
    git_cache: SourceCacheLane<'_>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    git_acquisitions: &mut GitAcquisitionCache<'_>,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    let root_request = PackageRootSourceRequest::ExternalLocal {
        requested_root: requested_root.to_path_buf(),
        source_context: source_context.clone(),
    };
    let mut external_roots: BTreeMap<PackageKey, PathBuf> =
        BTreeMap::from([(root.key().clone(), canonical_live_root.to_path_buf())]);

    resolve_registered_package_closure(
        root_request,
        root,
        closure_limits,
        workspace_cache,
        git_cache,
        local_cache,
        source_limits,
        &mut BTreeMap::new(),
        &mut external_roots,
        Some(&source_context),
        git_acquisitions,
    )
    .map_err(ResolveExternalLocalPackageClosureError::Closure)
}
