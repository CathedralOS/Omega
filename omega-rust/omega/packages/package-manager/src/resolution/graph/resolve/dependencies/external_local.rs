use super::super::cache::{SourceCacheLane, resolve_external_local_package_from_cache};
use super::super::errors::ResolveDependencySourceError;
use super::context::{WorkspaceContext, WorkspaceContextKind};
use crate::declarations::PackageKey;
use crate::resolution::source::PackageSourceCustody;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceLineage};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(super) fn resolve_external_dependency(
    requester: &PackageSourceCustody,
    location: &str,
    external_roots: &mut BTreeMap<PackageKey, PathBuf>,
    external_context: Option<&ExternalSourceContext>,
    local_cache: SourceCacheLane<'_>,
    source_limits: LocalSourceLimits,
) -> Result<PackageSourceCustody, ResolveDependencySourceError> {
    let requester_root = external_roots
        .get(requester.key())
        .cloned()
        .ok_or_else(|| ResolveDependencySourceError::UnknownExternalRoot {
            package: requester.key().clone(),
        })?;
    resolve_external_dependency_from_root(
        location,
        &requester_root,
        external_roots,
        external_context,
        local_cache,
        source_limits,
    )
}

pub(super) fn resolve_external_dependency_from_root(
    location: &str,
    requester_root: &Path,
    external_roots: &mut BTreeMap<PackageKey, PathBuf>,
    external_context: Option<&ExternalSourceContext>,
    local_cache: SourceCacheLane<'_>,
    source_limits: LocalSourceLimits,
) -> Result<PackageSourceCustody, ResolveDependencySourceError> {
    let target =
        crate::resolution::source::external_local_dependency_root(requester_root, location)
            .map_err(|reason| super::path::invalid_path(location, &reason))?;
    let source_context =
        external_context.ok_or(ResolveDependencySourceError::MissingExternalSourceContext)?;
    let resolved = resolve_external_local_package_from_cache(
        target,
        local_cache,
        source_limits,
        source_context.clone(),
    )?;
    register_external_root(
        external_roots,
        resolved.key(),
        resolved.source().canonical_live_root(),
    )?;
    Ok(resolved
        .into_custody()
        .with_compiled_review_cache_dir(local_cache.compiled_review_cache_dir()?))
}

pub(super) fn workspace_requester_root(
    requester: &PackageSourceCustody,
    context: &WorkspaceContext,
) -> Result<PathBuf, ResolveDependencySourceError> {
    let SourceLineage::Workspace(lineage) = requester.key().source_lineage() else {
        return Err(ResolveDependencySourceError::UnknownWorkspace {
            package: requester.key().clone(),
        });
    };
    let WorkspaceContextKind::Local { root, .. } = &context.kind else {
        return Err(ResolveDependencySourceError::UnknownWorkspace {
            package: requester.key().clone(),
        });
    };
    Ok(root.join(lineage.member_path().as_str()))
}

fn register_external_root(
    external_roots: &mut BTreeMap<PackageKey, PathBuf>,
    package: &PackageKey,
    canonical_live_root: &Path,
) -> Result<(), ResolveDependencySourceError> {
    if let Some(existing) = external_roots.get(package) {
        if existing != canonical_live_root {
            return Err(ResolveDependencySourceError::ConflictingExternalRoot {
                package: package.clone(),
            });
        }
    } else {
        external_roots.insert(package.clone(), canonical_live_root.to_path_buf());
    }
    Ok(())
}
