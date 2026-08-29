use super::projection::project_package_build;
use super::{ResolvePackageSourceError, ResolvedPackageSource};
use crate::source::identity::{
    GitCommitId, GitTreeId, ImmutableSourceResolution, PackageKey, SourceContentDigest,
    SourceLineage,
};
use crate::source::{
    GitSourceRequest, LocalSourceLimits, ResolvedGitSource, SourceResolverStorage,
};
use crate::source::{RetainedStorageLane, resolve_git_source_in_lane};
#[cfg(test)]
use std::path::Path;

/// Resolve a network Git request, then derive package identity only from the
/// canonical request lineage and the package declaration in the immutable
/// snapshot.
#[cfg(test)]
pub fn resolve_git_package_source(
    request: &GitSourceRequest,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir)?;
    resolve_git_package_source_with_storage(request, &storage, limits)
}

pub(crate) fn resolve_git_package_source_in_lane(
    request: &GitSourceRequest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let limits = limits.compiler_bounded();
    let lineage = request.lineage().clone();
    let source = resolve_git_source_in_lane(request, lane, limits)?;
    bind_git_package_source(lineage, source, limits)
}

pub fn resolve_git_package_source_with_storage(
    request: &GitSourceRequest,
    storage: &SourceResolverStorage,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    storage.verify_path_identity()?;
    let result = resolve_git_package_source_in_lane(request, storage.git_sources(), limits);
    storage.verify_path_identity()?;
    result
}

fn bind_git_package_source(
    lineage: SourceLineage,
    source: ResolvedGitSource,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let (declaration, dependency_requests) = project_package_build(source.snapshot_root(), false)?;
    let resolution = ImmutableSourceResolution::git(
        GitCommitId::parse_hex(source.commit())?,
        GitTreeId::parse_hex(source.tree())?,
        SourceContentDigest::derive(source.local().content_identity.as_bytes()),
    )?;

    Ok(ResolvedPackageSource::from_resolved_parts(
        PackageKey::new(declaration.name, lineage),
        resolution,
        source.snapshot_root().to_path_buf(),
        limits,
        dependency_requests,
        source,
    ))
}
