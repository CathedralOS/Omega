//! Resolve explicitly selected local roots outside a workspace.

use super::super::reconcile::{
    PackageRootSourceRequest, PackageSourceClosureLimits, ResolvedPackageSourceClosure,
};
use super::cache::{
    GitAcquisitionCache, SourceCacheLane, resolve_external_local_package_from_cache,
    resolve_external_local_project_from_cache,
};
use super::dependencies::resolve_registered_package_closure;
use super::errors::ResolveExternalLocalPackageClosureError;
use super::git_pins::GitResolutionOptions;
use crate::declarations::PackageKey;
use crate::resolution::source::{
    PackageSourceCustody, ResolvePackageSourceError, bind_staged_external_local_project_source,
};
#[cfg(test)]
use package_source::PrimaryGitChoices;
use package_source::local::staging::StagedLocalSnapshot;
use package_source::{ExternalSourceContext, SourceLineage};
use package_source::{LocalSourceLimits, SourceResolverStorage};
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
pub(crate) fn resolve_external_local_package_closure_from_hardened_base(
    live_root: impl AsRef<Path>,
    source_context: ExternalSourceContext,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir, PrimaryGitChoices::default())
        .map_err(|error| {
            ResolveExternalLocalPackageClosureError::Root(ResolvePackageSourceError::Source(error))
        })?;
    resolve_external_local_package_closure(
        live_root,
        source_context,
        &storage,
        source_limits,
        closure_limits,
    )
}

/// Resolve an external-local package closure beneath private resolver storage.
pub fn resolve_external_local_package_closure(
    live_root: impl AsRef<Path>,
    source_context: ExternalSourceContext,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    resolve_external_local_declared_closure(
        live_root.as_ref(),
        source_context,
        storage,
        source_limits,
        closure_limits,
        false,
        GitResolutionOptions::default(),
    )
}

/// Resolve a local compilation root and its complete declared dependency
/// closure. The root may be an application or a package; every dependency is
/// still required to be a package. The Git policy in `options` applies to
/// every dependency, including requests first discovered in transitive
/// sources; accepted pins must belong to this exact root request.
pub fn resolve_external_local_project_closure(
    live_root: impl AsRef<Path>,
    source_context: ExternalSourceContext,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    options: GitResolutionOptions<'_>,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    verify_pin_root(live_root.as_ref(), &source_context, options)?;
    resolve_external_local_declared_closure(
        live_root.as_ref(),
        source_context,
        storage,
        source_limits,
        closure_limits,
        true,
        options,
    )
}

/// Resolve a proposed local project using its original live directory for Path
/// dependencies. The caller retains the stage for the project-file transaction.
/// `options` carries the invocation-wide Git selection policy; accepted pins
/// (an install or selective update preserving unchanged accepted Git
/// requests) must belong to the original live root request and context.
pub fn resolve_staged_external_local_project_closure(
    stage: &StagedLocalSnapshot,
    source_context: ExternalSourceContext,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    options: GitResolutionOptions<'_>,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    verify_pin_root(stage.requested_root(), &source_context, options)?;
    resolve_staged_external_local_project(
        stage,
        source_context,
        storage,
        source_limits,
        closure_limits,
        &mut GitAcquisitionCache::with_options(options),
    )
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

fn resolve_staged_external_local_project(
    stage: &StagedLocalSnapshot,
    source_context: ExternalSourceContext,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    git_acquisitions: &mut GitAcquisitionCache<'_>,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    let source_error = |error| {
        ResolveExternalLocalPackageClosureError::Root(ResolvePackageSourceError::Source(error))
    };
    storage.verify_path_identity().map_err(source_error)?;
    let result =
        bind_staged_external_local_project_source(stage, source_limits, source_context.clone())
            .map_err(ResolveExternalLocalPackageClosureError::Root)
            .and_then(|root| {
                let source_limits = root.source_limits();
                let cache_dir = SourceCacheLane::Retained(storage.external_local_sources())
                    .checked_source_cache_dir()
                    .map_err(ResolveExternalLocalPackageClosureError::Root)?;
                resolve_bound_external_local_closure(
                    stage.requested_root(),
                    stage.canonical_live_root(),
                    source_context,
                    root.into_custody().with_checked_source_cache_dir(cache_dir),
                    SourceCacheLane::Retained(storage.external_local_sources()),
                    SourceCacheLane::Retained(storage.workspace_members()),
                    SourceCacheLane::Retained(storage.git_sources()),
                    source_limits,
                    closure_limits,
                    git_acquisitions,
                )
            });
    stage.verify_live_source_unchanged().map_err(source_error)?;
    storage.verify_path_identity().map_err(source_error)?;
    result
}

fn resolve_external_local_declared_closure(
    live_root: &Path,
    source_context: ExternalSourceContext,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    application_root_allowed: bool,
    options: GitResolutionOptions<'_>,
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
        &mut GitAcquisitionCache::with_options(options),
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
    git_acquisitions: &mut GitAcquisitionCache<'_>,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    let requested_root = live_root.to_path_buf();
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
    let canonical_live_root = root.source().canonical_live_root().to_path_buf();
    let cache_dir = local_cache
        .checked_source_cache_dir()
        .map_err(ResolveExternalLocalPackageClosureError::Root)?;
    resolve_bound_external_local_closure(
        &requested_root,
        &canonical_live_root,
        source_context,
        root.into_custody().with_checked_source_cache_dir(cache_dir),
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
