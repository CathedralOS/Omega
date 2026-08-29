//! Explicit local, workspace, and Git source policies for closure traversal.

use super::reconciliation::{
    PackageRootSourceRequest, PackageSourceClosureLimits, PackageSourceClosureResolutionError,
    PackageSourceCustody, ResolvedPackageSourceClosure, resolve_package_source_closure_with_limits,
};
use crate::declarations::dependency_projection::DependencySourceRequest;
use crate::source::acquisition::RetainedStorageLane;
use crate::source::identity::{
    ExternalSourceContext, PackageKey, SourceLineage, WorkspaceLineageIdentity, WorkspaceMemberPath,
};
use crate::source::package_resolution::{
    ResolvePackageSourceError, ResolvedPackageSource,
    resolve_external_local_package_source_in_lane, resolve_external_local_project_source_in_lane,
    resolve_git_package_source_in_lane, resolve_workspace_member_package_source_in_lane,
};
use crate::source::{
    GitSourceRequest, GitSourceRequestError, LocalSourceLimits, ResolvedGitSource,
    ResolvedLocalSnapshot, SourceResolverStorage,
};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ResolveWorkspacePackageClosureError {
    Root(ResolvePackageSourceError),
    RootRequestMismatch,
    Closure(PackageSourceClosureResolutionError<ResolveDependencySourceError>),
}

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub enum ResolveExternalLocalPackageClosureError {
    Root(ResolvePackageSourceError),
    RootRequestMismatch,
    Closure(PackageSourceClosureResolutionError<ResolveDependencySourceError>),
}

#[derive(Debug)]
pub enum ResolveGitPackageClosureError {
    Root(ResolvePackageSourceError),
    RootRequestMismatch,
    RootWorkspace(ResolveDependencySourceError),
    Closure(PackageSourceClosureResolutionError<ResolveDependencySourceError>),
}

impl fmt::Display for ResolveGitPackageClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(error) => write!(formatter, "cannot resolve root package: {error}"),
            Self::RootRequestMismatch => formatter
                .write_str("resolved root Git source does not match its exact validated request"),
            Self::RootWorkspace(error) => {
                write!(formatter, "cannot register root Git workspace: {error}")
            }
            Self::Closure(error) => write!(formatter, "cannot resolve package closure: {error}"),
        }
    }
}

impl std::error::Error for ResolveGitPackageClosureError {}

impl fmt::Display for ResolveExternalLocalPackageClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(error) => write!(formatter, "cannot resolve root package: {error}"),
            Self::RootRequestMismatch => {
                formatter.write_str("resolved external-local root does not match its exact request")
            }
            Self::Closure(error) => write!(formatter, "cannot resolve package closure: {error}"),
        }
    }
}

impl std::error::Error for ResolveExternalLocalPackageClosureError {}

impl fmt::Display for ResolveWorkspacePackageClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(error) => write!(formatter, "cannot resolve root package: {error}"),
            Self::RootRequestMismatch => formatter
                .write_str("resolved workspace root does not match its exact member request"),
            Self::Closure(error) => write!(formatter, "cannot resolve package closure: {error}"),
        }
    }
}

impl std::error::Error for ResolveWorkspacePackageClosureError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveDependencySourceError {
    InvalidPath {
        location: String,
        reason: String,
    },
    UnknownWorkspace {
        package: crate::source::identity::PackageKey,
    },
    ConflictingWorkspaceRoot {
        identity: WorkspaceLineageIdentity,
    },
    UnknownExternalRoot {
        package: PackageKey,
    },
    ConflictingExternalRoot {
        package: PackageKey,
    },
    MissingExternalSourceContext,
    InvalidGitRequest(GitSourceRequestError),
    Source(ResolvePackageSourceError),
}

impl fmt::Display for ResolveDependencySourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath { location, reason } => {
                write!(formatter, "invalid path dependency `{location}`: {reason}")
            }
            Self::UnknownWorkspace { package } => write!(
                formatter,
                "package `{}` has no registered immutable workspace root",
                package.name().as_str()
            ),
            Self::ConflictingWorkspaceRoot { .. } => formatter
                .write_str("one workspace lineage resolved to conflicting immutable source roots"),
            Self::UnknownExternalRoot { package } => write!(
                formatter,
                "external-local package `{}` has no registered live source root",
                package.name().as_str()
            ),
            Self::ConflictingExternalRoot { package } => write!(
                formatter,
                "external-local package `{}` resolved to conflicting live source roots",
                package.name().as_str()
            ),
            Self::MissingExternalSourceContext => formatter.write_str(
                "an external-local dependency requires an explicit consuming source context",
            ),
            Self::InvalidGitRequest(error) => error.fmt(formatter),
            Self::Source(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ResolveDependencySourceError {}

impl From<ResolvePackageSourceError> for ResolveDependencySourceError {
    fn from(error: ResolvePackageSourceError) -> Self {
        Self::Source(error)
    }
}

impl From<GitSourceRequestError> for ResolveDependencySourceError {
    fn from(error: GitSourceRequestError) -> Self {
        Self::InvalidGitRequest(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceContext {
    root_source: SourceLineage,
    root: PathBuf,
    allows_external_paths: bool,
}

#[derive(Clone, Copy)]
enum SourceCacheLane<'a> {
    Retained(&'a RetainedStorageLane),
}

/// Resolve one explicit workspace member and its complete Path/Git closure.
///
/// No parent-directory discovery occurs. Path requests are interpreted only
/// inside the explicit root or an immutable Git snapshot registered while
/// resolving this closure; an escape rejects before filesystem access.
#[cfg(test)]
pub(crate) fn resolve_workspace_package_closure(
    workspace_root_source: &SourceLineage,
    root_member_path: WorkspaceMemberPath,
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
    root_member_path: WorkspaceMemberPath,
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
    root_member_path: WorkspaceMemberPath,
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
    root_member_path: WorkspaceMemberPath,
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
    );
    storage.verify_path_identity().map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    result
}

fn resolve_workspace_package_closure_impl(
    workspace_root_source: &SourceLineage,
    root_member_path: WorkspaceMemberPath,
    live_workspace_root: &Path,
    workspace_cache: SourceCacheLane<'_>,
    git_cache: SourceCacheLane<'_>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    external_context: Option<&ExternalSourceContext>,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    let root_request = PackageRootSourceRequest::WorkspaceMember {
        workspace_root_source: workspace_root_source.clone(),
        member_path: root_member_path.clone(),
        requested_workspace_root: live_workspace_root.to_path_buf(),
    };
    let workspace_identity = WorkspaceLineageIdentity::from_root_source(workspace_root_source)
        .map_err(ResolvePackageSourceError::from)
        .map_err(ResolveWorkspacePackageClosureError::Root)?;
    let root = resolve_workspace_member_from_cache(
        workspace_root_source,
        root_member_path.clone(),
        live_workspace_root,
        workspace_cache,
        source_limits,
    )
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
    if root.source().canonical_live_root != expected_member_root
        || root.key().source_lineage()
            != &SourceLineage::Workspace(crate::source::identity::WorkspaceMemberLineage::new(
                workspace_identity.clone(),
                root_member_path,
            ))
    {
        return Err(ResolveWorkspacePackageClosureError::RootRequestMismatch);
    }
    let mut workspaces = BTreeMap::from([(
        workspace_identity,
        WorkspaceContext {
            root_source: workspace_root_source.clone(),
            root: canonical_workspace_root,
            allows_external_paths: true,
        },
    )]);

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
    )
    .map_err(ResolveWorkspacePackageClosureError::Closure)
}

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

fn git_root_request_matches(
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
/// Resolve a local project closure beneath private resolver storage.
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
    if root.source().requested_root != requested_root
        || !matches!(
            root.key().source_lineage(),
            SourceLineage::ExternalLocal(lineage)
                if lineage.source_context() == &source_context
        )
    {
        return Err(ResolveExternalLocalPackageClosureError::RootRequestMismatch);
    }
    let mut external_roots = BTreeMap::from([(
        root.key().clone(),
        root.source().canonical_live_root.clone(),
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

fn resolve_git_from_cache(
    request: &GitSourceRequest,
    cache: SourceCacheLane<'_>,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    match cache {
        SourceCacheLane::Retained(lane) => {
            resolve_git_package_source_in_lane(request, lane, limits)
        }
    }
}

fn resolve_external_local_package_from_cache(
    source_root: impl AsRef<Path>,
    cache: SourceCacheLane<'_>,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    match cache {
        SourceCacheLane::Retained(lane) => {
            resolve_external_local_package_source_in_lane(source_root, lane, limits, source_context)
        }
    }
}

fn resolve_external_local_project_from_cache(
    source_root: impl AsRef<Path>,
    cache: SourceCacheLane<'_>,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    match cache {
        SourceCacheLane::Retained(lane) => {
            resolve_external_local_project_source_in_lane(source_root, lane, limits, source_context)
        }
    }
}

fn resolve_workspace_member_from_cache(
    workspace_root_source: &SourceLineage,
    member_path: WorkspaceMemberPath,
    live_workspace_root: impl AsRef<Path>,
    cache: SourceCacheLane<'_>,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    match cache {
        SourceCacheLane::Retained(lane) => resolve_workspace_member_package_source_in_lane(
            workspace_root_source,
            member_path,
            live_workspace_root,
            lane,
            limits,
        ),
    }
}

fn resolve_registered_package_closure(
    root_request: PackageRootSourceRequest,
    root: PackageSourceCustody,
    closure_limits: PackageSourceClosureLimits,
    workspace_cache: SourceCacheLane<'_>,
    git_cache: SourceCacheLane<'_>,
    external_local_cache: SourceCacheLane<'_>,
    source_limits: LocalSourceLimits,
    workspaces: &mut BTreeMap<WorkspaceLineageIdentity, WorkspaceContext>,
    external_roots: &mut BTreeMap<PackageKey, PathBuf>,
    external_context: Option<&ExternalSourceContext>,
) -> Result<
    ResolvedPackageSourceClosure,
    PackageSourceClosureResolutionError<ResolveDependencySourceError>,
> {
    resolve_package_source_closure_with_limits(
        root_request,
        root,
        closure_limits,
        |requester, request| match request {
            DependencySourceRequest::Git {
                repository,
                revision,
                ..
            } => {
                let resolved = resolve_git_from_cache(
                    &GitSourceRequest::new(repository.clone(), Some(revision.clone()))?,
                    git_cache,
                    source_limits,
                )?;
                register_workspace(
                    workspaces,
                    resolved.key().source_lineage(),
                    resolved.snapshot_root(),
                )?;
                Ok(resolved.into_custody())
            }
            DependencySourceRequest::Path { location, .. } => {
                if matches!(
                    requester.key().source_lineage(),
                    SourceLineage::ExternalLocal(_)
                ) {
                    return resolve_external_dependency(
                        requester,
                        location,
                        external_roots,
                        external_context,
                        external_local_cache,
                        source_limits,
                    );
                }
                let (workspace_identity, base) = requester_workspace(requester, workspaces)?;
                let context = workspaces.get(&workspace_identity).ok_or_else(|| {
                    ResolveDependencySourceError::UnknownWorkspace {
                        package: requester.key().clone(),
                    }
                })?;
                match normalize_member_path(base.as_deref(), location) {
                    Ok(member_path) => resolve_workspace_member_from_cache(
                        &context.root_source,
                        member_path,
                        &context.root,
                        workspace_cache,
                        source_limits,
                    )
                    .map(|resolved| resolved.into_custody())
                    .map_err(ResolveDependencySourceError::from),
                    Err(_)
                        if context.allows_external_paths
                            && external_context.is_some()
                            && workspace_path_escapes(base.as_deref(), location) =>
                    {
                        let requester_root = workspace_requester_root(requester, context)?;
                        resolve_external_dependency_from_root(
                            location,
                            &requester_root,
                            external_roots,
                            external_context,
                            external_local_cache,
                            source_limits,
                        )
                    }
                    Err(error) => Err(error),
                }
            }
        },
    )
}

fn register_workspace(
    workspaces: &mut BTreeMap<WorkspaceLineageIdentity, WorkspaceContext>,
    root_source: &SourceLineage,
    root: &Path,
) -> Result<WorkspaceLineageIdentity, ResolveDependencySourceError> {
    let identity = WorkspaceLineageIdentity::from_root_source(root_source)
        .map_err(ResolvePackageSourceError::from)?;
    let context = WorkspaceContext {
        root_source: root_source.clone(),
        root: root.to_path_buf(),
        allows_external_paths: false,
    };
    if let Some(existing) = workspaces.get(&identity) {
        if existing != &context {
            return Err(ResolveDependencySourceError::ConflictingWorkspaceRoot { identity });
        }
    } else {
        workspaces.insert(identity.clone(), context);
    }
    Ok(identity)
}

fn resolve_external_dependency(
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

fn resolve_external_dependency_from_root(
    location: &str,
    requester_root: &Path,
    external_roots: &mut BTreeMap<PackageKey, PathBuf>,
    external_context: Option<&ExternalSourceContext>,
    local_cache: SourceCacheLane<'_>,
    source_limits: LocalSourceLimits,
) -> Result<PackageSourceCustody, ResolveDependencySourceError> {
    if location.is_empty() || location.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(invalid_path(
            location,
            "external-local path must be nonempty and contain no control bytes",
        ));
    }
    let source_context =
        external_context.ok_or(ResolveDependencySourceError::MissingExternalSourceContext)?;
    let authored = Path::new(location);
    let target = if authored.is_absolute() {
        authored.to_path_buf()
    } else {
        requester_root.join(authored)
    };
    let resolved = resolve_external_local_package_from_cache(
        target,
        local_cache,
        source_limits,
        source_context.clone(),
    )?;
    register_external_root(
        external_roots,
        resolved.key(),
        &resolved.source().canonical_live_root,
    )?;
    Ok(resolved.into_custody())
}

fn workspace_requester_root(
    requester: &PackageSourceCustody,
    context: &WorkspaceContext,
) -> Result<PathBuf, ResolveDependencySourceError> {
    let SourceLineage::Workspace(lineage) = requester.key().source_lineage() else {
        return Err(ResolveDependencySourceError::UnknownWorkspace {
            package: requester.key().clone(),
        });
    };
    Ok(context.root.join(lineage.member_path().as_str()))
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

fn requester_workspace(
    requester: &PackageSourceCustody,
    workspaces: &mut BTreeMap<WorkspaceLineageIdentity, WorkspaceContext>,
) -> Result<(WorkspaceLineageIdentity, Option<String>), ResolveDependencySourceError> {
    match requester.key().source_lineage() {
        SourceLineage::Workspace(lineage) => Ok((
            lineage.workspace_identity().clone(),
            Some(lineage.member_path().as_str().to_owned()),
        )),
        lineage @ (SourceLineage::GitHub(_)
        | SourceLineage::GitLab(_)
        | SourceLineage::Git(_)
        | SourceLineage::ExternalLocal(_)) => {
            let identity = register_workspace(workspaces, lineage, requester.snapshot_root())?;
            Ok((identity, None))
        }
    }
}

fn normalize_member_path(
    requester_member: Option<&str>,
    location: &str,
) -> Result<WorkspaceMemberPath, ResolveDependencySourceError> {
    if location.is_empty()
        || location.starts_with('/')
        || location.ends_with('/')
        || location.contains('\\')
        || location.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(invalid_path(
            location,
            "path must be a portable relative location",
        ));
    }

    let mut components = requester_member
        .map(|member| member.split('/').map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_default();
    for component in location.split('/') {
        match component {
            "" => return Err(invalid_path(location, "path contains an empty component")),
            "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(invalid_path(
                        location,
                        "path escapes its registered workspace",
                    ));
                }
            }
            component => components.push(component.to_owned()),
        }
    }
    if components.is_empty() {
        return Err(invalid_path(
            location,
            "path resolves to the workspace root",
        ));
    }
    WorkspaceMemberPath::parse(&components.join("/"))
        .map_err(|error| invalid_path(location, &error.to_string()))
}

fn workspace_path_escapes(requester_member: Option<&str>, location: &str) -> bool {
    if Path::new(location).is_absolute() {
        return true;
    }
    if location.is_empty()
        || location.ends_with('/')
        || location.contains('\\')
        || location.bytes().any(|byte| byte.is_ascii_control())
    {
        return false;
    }
    let mut depth = requester_member
        .map(|member| member.split('/').count())
        .unwrap_or(0);
    for component in location.split('/') {
        match component {
            "" => return false,
            "." => {}
            ".." if depth == 0 => return true,
            ".." => depth -= 1,
            _ => depth += 1,
        }
    }
    false
}

fn invalid_path(location: &str, reason: &str) -> ResolveDependencySourceError {
    ResolveDependencySourceError::InvalidPath {
        location: location.to_owned(),
        reason: reason.to_owned(),
    }
}
