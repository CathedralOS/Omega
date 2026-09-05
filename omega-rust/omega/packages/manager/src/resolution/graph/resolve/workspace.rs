//! Resolve explicitly selected workspace members.

use super::super::reconcile::{
    PackageRootSourceRequest, PackageSourceClosureLimits, ResolvedPackageSourceClosure,
};
use super::cache::{
    GitAcquisitionCache, SourceCacheLane, resolve_workspace_member_from_cache,
    resolve_workspace_member_project_from_cache,
};
use super::dependencies::{WorkspaceContext, resolve_registered_package_closure};
use super::errors::ResolveWorkspacePackageClosureError;
use crate::resolution::source::ResolvePackageSourceError;
use package_source::{
    ExternalSourceContext, SourceLineage, SourceRelativePath, WorkspaceLineageIdentity,
    WorkspaceMemberLineage,
};
use package_source::{LocalSourceLimits, SourceResolverStorage};
use std::collections::BTreeMap;
use std::path::Path;

/// Resolve one explicit workspace member and its complete Path/Git closure.
///
/// No parent-directory discovery occurs. Path requests are interpreted only
/// inside the explicit root or an immutable Git snapshot registered while
/// resolving this closure; an escape rejects before filesystem access.
#[cfg(test)]
pub(crate) fn resolve_workspace_package_closure(
    workspace_root_source: &SourceLineage,
    root_member_path: SourceRelativePath,
    live_workspace_root: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir).map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    resolve_workspace_package_closure_with_storage(
        workspace_root_source,
        root_member_path,
        live_workspace_root,
        &storage,
        source_limits,
        closure_limits,
    )
}

/// Resolve a workspace closure beneath the manager-owned private source root.
pub fn resolve_workspace_package_closure_with_storage(
    workspace_root_source: &SourceLineage,
    root_member_path: SourceRelativePath,
    live_workspace_root: impl AsRef<Path>,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    storage.verify_path_identity().map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    let result = resolve_workspace_package_closure_impl(
        workspace_root_source,
        root_member_path,
        live_workspace_root.as_ref(),
        SourceCacheLane::Retained(storage.workspace_members()),
        SourceCacheLane::Retained(storage.git_sources()),
        source_limits,
        closure_limits,
        None,
        false,
    );
    storage.verify_path_identity().map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    result
}

/// Resolve a selected workspace project root. The root may be a package or an
/// application; dependencies remain package-only.
pub fn resolve_workspace_project_closure_with_storage(
    workspace_root_source: &SourceLineage,
    root_member_path: SourceRelativePath,
    live_workspace_root: impl AsRef<Path>,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    storage.verify_path_identity().map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    let result = resolve_workspace_package_closure_impl(
        workspace_root_source,
        root_member_path,
        live_workspace_root.as_ref(),
        SourceCacheLane::Retained(storage.workspace_members()),
        SourceCacheLane::Retained(storage.git_sources()),
        source_limits,
        closure_limits,
        None,
        true,
    );
    storage.verify_path_identity().map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    result
}

/// Resolve an explicit workspace closure while allowing a Path request that
/// leaves that live workspace to become a context-bound external-local source.
///
/// The supplied context is identity, not ambient authority: no lock or parent
/// workspace is discovered. Path requests originating in fetched Git snapshots
/// remain confined to those immutable snapshots.
#[cfg(test)]
pub(crate) fn resolve_workspace_package_closure_in_context(
    workspace_root_source: &SourceLineage,
    root_member_path: SourceRelativePath,
    live_workspace_root: impl AsRef<Path>,
    source_context: ExternalSourceContext,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir).map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    resolve_workspace_package_closure_in_context_with_storage(
        workspace_root_source,
        root_member_path,
        live_workspace_root,
        source_context,
        &storage,
        source_limits,
        closure_limits,
    )
}

/// Resolve a context-enabled workspace closure beneath private resolver storage.
pub fn resolve_workspace_package_closure_in_context_with_storage(
    workspace_root_source: &SourceLineage,
    root_member_path: SourceRelativePath,
    live_workspace_root: impl AsRef<Path>,
    source_context: ExternalSourceContext,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    storage.verify_path_identity().map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    let result = resolve_workspace_package_closure_impl(
        workspace_root_source,
        root_member_path,
        live_workspace_root.as_ref(),
        SourceCacheLane::Retained(storage.workspace_members()),
        SourceCacheLane::Retained(storage.git_sources()),
        source_limits,
        closure_limits,
        Some(&source_context),
        false,
    );
    storage.verify_path_identity().map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    result
}

/// Context-enabled project-root variant for a workspace member that may be an
/// application while external path dependencies remain explicitly scoped.
#[allow(clippy::too_many_arguments)]
pub fn resolve_workspace_project_closure_in_context_with_storage(
    workspace_root_source: &SourceLineage,
    root_member_path: SourceRelativePath,
    live_workspace_root: impl AsRef<Path>,
    source_context: ExternalSourceContext,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    storage.verify_path_identity().map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    let result = resolve_workspace_package_closure_impl(
        workspace_root_source,
        root_member_path,
        live_workspace_root.as_ref(),
        SourceCacheLane::Retained(storage.workspace_members()),
        SourceCacheLane::Retained(storage.git_sources()),
        source_limits,
        closure_limits,
        Some(&source_context),
        true,
    );
    storage.verify_path_identity().map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    result
}

#[allow(clippy::too_many_arguments)]
fn resolve_workspace_package_closure_impl(
    workspace_root_source: &SourceLineage,
    root_member_path: SourceRelativePath,
    live_workspace_root: &Path,
    workspace_cache: SourceCacheLane<'_>,
    git_cache: SourceCacheLane<'_>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    external_context: Option<&ExternalSourceContext>,
    application_root_allowed: bool,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    let root_request = PackageRootSourceRequest::WorkspaceMember {
        workspace_root_source: workspace_root_source.clone(),
        member_path: root_member_path.clone(),
        requested_workspace_root: live_workspace_root.to_path_buf(),
    };
    let workspace_identity = WorkspaceLineageIdentity::from_root_source(workspace_root_source)
        .map_err(ResolvePackageSourceError::from)
        .map_err(ResolveWorkspacePackageClosureError::Root)?;
    let root = if application_root_allowed {
        resolve_workspace_member_project_from_cache(
            workspace_root_source,
            root_member_path.clone(),
            live_workspace_root,
            workspace_cache,
            source_limits,
        )
    } else {
        resolve_workspace_member_from_cache(
            workspace_root_source,
            root_member_path.clone(),
            live_workspace_root,
            workspace_cache,
            source_limits,
        )
    }
    .map_err(ResolveWorkspacePackageClosureError::Root)?;

    let canonical_workspace_root = live_workspace_root.canonicalize().map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::WorkspacePath {
            path: live_workspace_root.to_path_buf(),
            message: error.to_string(),
        })
    })?;
    let requested_member_root = canonical_workspace_root.join(root_member_path.as_str());
    let expected_member_root = requested_member_root.canonicalize().map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::WorkspacePath {
            path: requested_member_root,
            message: error.to_string(),
        })
    })?;
    if root.source().canonical_live_root() != expected_member_root
        || root.key().source_lineage()
            != &SourceLineage::Workspace(WorkspaceMemberLineage::new(
                workspace_identity.clone(),
                root_member_path,
            ))
    {
        return Err(ResolveWorkspacePackageClosureError::RootRequestMismatch);
    }
    let mut workspaces = BTreeMap::from([(
        workspace_identity,
        WorkspaceContext::local(
            workspace_root_source.clone(),
            canonical_workspace_root,
            true,
        ),
    )]);
    let mut git_acquisitions = GitAcquisitionCache::default();

    resolve_registered_package_closure(
        root_request,
        root.into_custody(),
        closure_limits,
        workspace_cache,
        git_cache,
        workspace_cache,
        source_limits,
        &mut workspaces,
        &mut BTreeMap::new(),
        external_context,
        &mut git_acquisitions,
    )
    .map_err(ResolveWorkspacePackageClosureError::Closure)
}
