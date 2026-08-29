use super::projection::project_package_build;
use super::{ResolvePackageSourceError, ResolvedPackageSource};
use crate::resolution::acquisition::{RetainedStorageLane, resolve_local_source_snapshot_in_lane};
use crate::resolution::identity::{
    ExternalLocalLineage, ExternalSourceContext, ImmutableSourceResolution, PackageKey,
    SourceContentDigest, SourceLineage,
};
use crate::resolution::{LocalSourceLimits, ResolvedLocalSnapshot, SourceResolverStorage};
use std::path::Path;

/// Snapshot a non-workspace local development source and bind its canonical
/// path to an explicit consuming context. Such lineage is intentionally
/// non-portable and cannot impersonate a workspace or network source.
#[cfg(test)]
pub fn resolve_external_local_package_source(
    source_root: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir)?;
    resolve_external_local_package_source_with_storage(
        source_root,
        &storage,
        limits,
        source_context,
    )
}

/// Resolve a local project root selected for execution.
///
/// Unlike a dependency source, the root of a compilation may declare either
/// `builder.application(...)` or `builder.package(...)`. Dependencies reached
/// from it remain package-only. Keeping that distinction here prevents the CLI
/// from falling back to directory-shaped import lookup merely because its root
/// is an application rather than a library package.
pub(crate) fn resolve_external_local_package_source_in_lane(
    source_root: impl AsRef<Path>,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    resolve_external_local_declared_source_in_lane(
        source_root.as_ref(),
        lane,
        limits,
        source_context,
        false,
    )
}

pub(crate) fn resolve_external_local_project_source_in_lane(
    source_root: impl AsRef<Path>,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    resolve_external_local_declared_source_in_lane(
        source_root.as_ref(),
        lane,
        limits,
        source_context,
        true,
    )
}

pub fn resolve_external_local_package_source_with_storage(
    source_root: impl AsRef<Path>,
    storage: &SourceResolverStorage,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    storage.verify_path_identity()?;
    let result = resolve_external_local_package_source_in_lane(
        source_root,
        storage.external_local_sources(),
        limits,
        source_context,
    );
    storage.verify_path_identity()?;
    result
}

pub fn resolve_external_local_project_source_with_storage(
    source_root: impl AsRef<Path>,
    storage: &SourceResolverStorage,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    storage.verify_path_identity()?;
    let result = resolve_external_local_project_source_in_lane(
        source_root,
        storage.external_local_sources(),
        limits,
        source_context,
    );
    storage.verify_path_identity()?;
    result
}

fn resolve_external_local_declared_source_in_lane(
    source_root: &Path,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
    application_root_allowed: bool,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    let limits = limits.compiler_bounded();
    let source = resolve_local_source_snapshot_in_lane(source_root, lane, limits)?;
    bind_external_local_declared_source(source, limits, source_context, application_root_allowed)
}

fn bind_external_local_declared_source(
    source: ResolvedLocalSnapshot,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
    application_root_allowed: bool,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    let lineage = SourceLineage::ExternalLocal(ExternalLocalLineage::canonicalize(
        &source.canonical_live_root,
        source_context,
    )?);
    let (declaration, dependency_requests) =
        project_package_build(&source.snapshot_root, application_root_allowed)?;
    let resolution = ImmutableSourceResolution::external_local(SourceContentDigest::derive(
        source.normalized.content_identity.as_bytes(),
    ));

    Ok(ResolvedPackageSource::from_resolved_parts(
        PackageKey::new(declaration.name, lineage),
        resolution,
        source.snapshot_root.clone(),
        limits,
        dependency_requests,
        source,
    ))
}
