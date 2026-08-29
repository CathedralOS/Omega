use super::projection::project_package_build;
use super::{ResolvePackageSourceError, ResolvedPackageSource};
use crate::resolution::identity::{
    GitCommitId, GitTreeId, ImmutableSourceResolution, PackageKey, SourceContentDigest,
    SourceLineage,
};
use crate::source::{GitSourceRequest, LocalSourceLimits, ResolvedGitSource, resolve_git_source};
use std::path::Path;

/// Resolve a network Git request, then derive package identity only from the
/// canonical request lineage and the package declaration in the immutable
/// snapshot.
pub fn resolve_git_package_source(
    request: &GitSourceRequest,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let limits = limits.compiler_bounded();
    let lineage = request.lineage().clone();
    let source = resolve_git_source(request, cache_dir, limits)?;
    bind_git_package_source(lineage, source, limits)
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
