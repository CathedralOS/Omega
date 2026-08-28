use crate::closure_resolution::{
    PackageRootSourceRequest, PackageSourceClosureLimits, PackageSourceClosureResolutionError,
    PackageSourceCustody, ResolvedPackageSourceClosure, resolve_package_source_closure_with_limits,
};
use crate::dependency_projection::DependencySourceRequest;
use crate::identity::{
    ExternalSourceContext, PackageKey, SourceLineage, WorkspaceLineageIdentity, WorkspaceMemberPath,
};
use crate::package_source::{
    ResolvePackageSourceError, resolve_external_local_package_source,
    resolve_external_local_project_source, resolve_git_package_source,
    resolve_workspace_member_package_source,
};
use crate::source::{
    GitSourceRequest, GitSourceRequestError, LocalSourceLimits, ResolvedGitSource,
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
        package: crate::identity::PackageKey,
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

/// Resolve one explicit workspace member and its complete Path/Git closure.
///
/// No parent-directory discovery occurs. Path requests are interpreted only
/// inside the explicit root or an immutable Git snapshot registered while
/// resolving this closure; an escape rejects before filesystem access.
pub fn resolve_workspace_package_closure(
    workspace_root_source: &SourceLineage,
    root_member_path: WorkspaceMemberPath,
    live_workspace_root: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    resolve_workspace_package_closure_impl(
        workspace_root_source,
        root_member_path,
        live_workspace_root.as_ref(),
        cache_dir.as_ref(),
        source_limits,
        closure_limits,
        None,
    )
}

/// Resolve an explicit workspace closure while allowing a Path request that
/// leaves that live workspace to become a context-bound external-local source.
///
/// The supplied context is identity, not ambient authority: no lock or parent
/// workspace is discovered. Path requests originating in fetched Git snapshots
/// remain confined to those immutable snapshots.
pub fn resolve_workspace_package_closure_in_context(
    workspace_root_source: &SourceLineage,
    root_member_path: WorkspaceMemberPath,
    live_workspace_root: impl AsRef<Path>,
    source_context: ExternalSourceContext,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    resolve_workspace_package_closure_impl(
        workspace_root_source,
        root_member_path,
        live_workspace_root.as_ref(),
        cache_dir.as_ref(),
        source_limits,
        closure_limits,
        Some(&source_context),
    )
}

fn resolve_workspace_package_closure_impl(
    workspace_root_source: &SourceLineage,
    root_member_path: WorkspaceMemberPath,
    live_workspace_root: &Path,
    cache_dir: &Path,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    external_context: Option<&ExternalSourceContext>,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    let root_request = PackageRootSourceRequest::WorkspaceMember {
        workspace_root_source: workspace_root_source.clone(),
        member_path: root_member_path.clone(),
        requested_workspace_root: live_workspace_root.to_path_buf(),
    };
    let workspace_cache = cache_dir.join("workspace-members");
    let git_cache = cache_dir.join("git-sources");
    let workspace_identity = WorkspaceLineageIdentity::from_root_source(workspace_root_source)
        .map_err(ResolvePackageSourceError::from)
        .map_err(ResolveWorkspacePackageClosureError::Root)?;
    let root = resolve_workspace_member_package_source(
        workspace_root_source,
        root_member_path.clone(),
        live_workspace_root,
        &workspace_cache,
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
            != &SourceLineage::Workspace(crate::identity::WorkspaceMemberLineage::new(
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
        &workspace_cache,
        &git_cache,
        &workspace_cache,
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
pub fn resolve_git_package_closure(
    request: &GitSourceRequest,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveGitPackageClosureError> {
    let cache_dir = cache_dir.as_ref();
    let workspace_cache = cache_dir.join("workspace-members");
    let git_cache = cache_dir.join("git-sources");
    let local_cache = cache_dir.join("external-local-sources");
    let root = resolve_git_package_source(request, &git_cache, source_limits)
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
        &workspace_cache,
        &git_cache,
        &local_cache,
        source_limits,
        &mut workspaces,
        &mut BTreeMap::new(),
        None,
    )
    .map_err(ResolveGitPackageClosureError::Closure)
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
pub fn resolve_external_local_package_closure(
    live_root: impl AsRef<Path>,
    source_context: ExternalSourceContext,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    resolve_external_local_declared_closure(
        live_root.as_ref(),
        source_context,
        cache_dir.as_ref(),
        source_limits,
        closure_limits,
        false,
    )
}

/// Resolve a local compilation root and its complete declared dependency
/// closure. The root may be an application or a package; every dependency is
/// still required to be a package.
pub fn resolve_external_local_project_closure(
    live_root: impl AsRef<Path>,
    source_context: ExternalSourceContext,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    resolve_external_local_declared_closure(
        live_root.as_ref(),
        source_context,
        cache_dir.as_ref(),
        source_limits,
        closure_limits,
        true,
    )
}

fn resolve_external_local_declared_closure(
    live_root: &Path,
    source_context: ExternalSourceContext,
    cache_dir: &Path,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    application_root_allowed: bool,
) -> Result<ResolvedPackageSourceClosure, ResolveExternalLocalPackageClosureError> {
    let requested_root = live_root.to_path_buf();
    let root_request = PackageRootSourceRequest::ExternalLocal {
        requested_root: requested_root.clone(),
        source_context: source_context.clone(),
    };
    let local_cache = cache_dir.join("external-local-sources");
    let workspace_cache = cache_dir.join("workspace-members");
    let git_cache = cache_dir.join("git-sources");
    let root = if application_root_allowed {
        resolve_external_local_project_source(
            &requested_root,
            &local_cache,
            source_limits,
            source_context.clone(),
        )
    } else {
        resolve_external_local_package_source(
            &requested_root,
            &local_cache,
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
        &workspace_cache,
        &git_cache,
        &local_cache,
        source_limits,
        &mut BTreeMap::new(),
        &mut external_roots,
        Some(&source_context),
    )
    .map_err(ResolveExternalLocalPackageClosureError::Closure)
}

fn resolve_registered_package_closure(
    root_request: PackageRootSourceRequest,
    root: PackageSourceCustody,
    closure_limits: PackageSourceClosureLimits,
    workspace_cache: &Path,
    git_cache: &Path,
    external_local_cache: &Path,
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
                let resolved = resolve_git_package_source(
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
                    Ok(member_path) => resolve_workspace_member_package_source(
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
    local_cache: &Path,
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
    local_cache: &Path,
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
    let resolved = resolve_external_local_package_source(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GitTransportProfile, PackageSourceClosureLimitKind, PackageSourceReviewLimits,
        PackageTriageDisposition, PackageTriageReason, ReviewOnlyCapabilityConflictChange,
        ReviewOnlyCapabilityConflictLimits, assemble_update_source_review,
        compare_review_only_capabilities, compile_resolved_package_reviews, triage_review_update,
    };
    use omega_package_review::{
        PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
        PackageReviewDangerousAuthorityClass, PackageReviewNominalOwner,
        PackageReviewSourceLocationRole,
    };
    use std::collections::BTreeSet;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../tests/fixtures/packages")
    }

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time follows Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omega-package-source-adapter-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn fixture_lineage() -> SourceLineage {
        SourceLineage::git("https://github.com/CathedralOS/package-fixtures.git")
            .expect("fixture lineage")
    }

    fn write_package(root: &Path, name: &str, dependency: Option<&str>) {
        std::fs::create_dir_all(root).expect("create package");
        let dependency = dependency
            .map(|location| {
                let location = location.replace('\\', "\\\\").replace('"', "\\\"");
                format!("    builder.depend(Source::Path {{ location: \"{location}\" }});\n")
            })
            .unwrap_or_default();
        std::fs::write(
            root.join("build.omg"),
            format!(
                "machine build(builder: &mut Build) {{\n    builder.package(\"{name}\");\n{dependency}}}\n"
            ),
        )
        .expect("write build file");
        std::fs::write(root.join("main.omg"), "machine root() {}\n").expect("write source");
    }

    fn run_test_git<I, S>(directory: &Path, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let output = Command::new("git")
            .current_dir(directory)
            .args(args)
            .output()
            .expect("spawn test Git");
        assert!(
            output.status.success(),
            "test Git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn test_git_head(directory: &Path) -> String {
        let output = Command::new("git")
            .current_dir(directory)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("read test Git HEAD");
        assert!(
            output.status.success(),
            "test Git rev-parse failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("Git object ID is UTF-8")
            .trim()
            .to_owned()
    }

    #[test]
    fn resolves_explicit_workspace_path_closure() {
        let cache = temp_root("fixture-cache");
        let closure = resolve_workspace_package_closure(
            &fixture_lineage(),
            WorkspaceMemberPath::parse("graph-workbench").expect("root member"),
            fixture_root(),
            &cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .expect("resolve local fixture closure");

        assert_eq!(closure.graph().packages().len(), 3);
        let root = closure
            .graph()
            .package(closure.graph().root())
            .expect("root package");
        let aliases = root
            .dependencies()
            .iter()
            .map(|dependency| dependency.alias().as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            aliases,
            std::collections::BTreeSet::from(["arithmetic_kernels", "file_journal"])
        );
        let root_binding = closure.source_requests().root();
        let PackageRootSourceRequest::WorkspaceMember {
            workspace_root_source,
            member_path,
            requested_workspace_root,
        } = root_binding.request()
        else {
            panic!("workspace adapter retains the workspace root request")
        };
        assert_eq!(workspace_root_source, &fixture_lineage());
        assert_eq!(member_path.as_str(), "graph-workbench");
        assert_eq!(requested_workspace_root, &fixture_root());
        assert_eq!(root_binding.selected().key(), closure.graph().root());
        assert_eq!(closure.source_requests().dependencies().count(), 2);

        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn resolves_nested_paths_relative_to_each_requester() {
        let workspace = temp_root("nested-workspace");
        let cache = temp_root("nested-cache");
        write_package(
            &workspace.join("packages/root"),
            "root-package",
            Some("../middle"),
        );
        write_package(
            &workspace.join("packages/middle"),
            "middle-package",
            Some("../leaf"),
        );
        write_package(&workspace.join("packages/leaf"), "leaf-package", None);

        let closure = resolve_workspace_package_closure(
            &fixture_lineage(),
            WorkspaceMemberPath::parse("packages/root").expect("root member"),
            &workspace,
            &cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .expect("resolve nested workspace closure");

        assert_eq!(closure.graph().packages().len(), 3);
        assert!(closure.custodies().iter().any(|custody| {
            custody.key().name().as_str() == "leaf-package"
                && matches!(custody.key().source_lineage(), SourceLineage::Workspace(_))
        }));

        let _ = std::fs::remove_dir_all(workspace);
        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn resolves_external_local_closure_across_directory_boundaries_in_one_context() {
        let sources = temp_root("external-sources");
        let first_cache = temp_root("external-first-cache");
        let second_cache = temp_root("external-second-cache");
        write_package(&sources.join("root"), "root-package", Some("../middle"));
        let leaf = sources.join("leaf");
        let leaf_location = leaf.display().to_string();
        write_package(
            &sources.join("middle"),
            "middle-package",
            Some(&leaf_location),
        );
        write_package(&leaf, "leaf-package", None);
        let first_context = ExternalSourceContext::derive(b"first-consuming-lock");

        let first = resolve_external_local_package_closure(
            sources.join("root"),
            first_context.clone(),
            &first_cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .expect("resolve context-bound external closure");

        assert_eq!(first.graph().packages().len(), 3);
        assert!(first.custodies().iter().all(|custody| {
            matches!(
                custody.key().source_lineage(),
                SourceLineage::ExternalLocal(lineage)
                    if lineage.source_context() == &first_context
            )
        }));
        let first_root_binding = first.source_requests().root();
        let PackageRootSourceRequest::ExternalLocal {
            requested_root,
            source_context,
        } = first_root_binding.request()
        else {
            panic!("external adapter retains its root request")
        };
        assert_eq!(requested_root, &sources.join("root"));
        assert_eq!(source_context, &first_context);

        let second_context = ExternalSourceContext::derive(b"second-consuming-lock");
        let second = resolve_external_local_package_closure(
            sources.join("root"),
            second_context,
            &second_cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .expect("resolve same sources in a different consuming context");
        for first_custody in first.custodies() {
            let second_custody = second
                .custodies()
                .iter()
                .find(|custody| custody.key().name() == first_custody.key().name())
                .expect("same declared package in second closure");
            assert_ne!(first_custody.key(), second_custody.key());
            assert_eq!(first_custody.resolution(), second_custody.resolution());
        }

        let _ = std::fs::remove_dir_all(sources);
        let _ = std::fs::remove_dir_all(first_cache);
        let _ = std::fs::remove_dir_all(second_cache);
    }

    #[test]
    fn resolves_repository_root_git_closure_and_retains_the_exact_request() {
        let repository = temp_root("git-root-repository");
        let cache = temp_root("git-root-cache");
        write_package(&repository, "network-root", None);
        run_test_git(&repository, ["init", "--quiet"]);
        run_test_git(
            &repository,
            ["config", "user.email", "omega@example.invalid"],
        );
        run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
        run_test_git(&repository, ["add", "."]);
        run_test_git(&repository, ["commit", "--quiet", "-m", "root"]);
        let request = GitSourceRequest::for_local_test_repository_with_lineage(
            &repository,
            None,
            "https://github.com/CathedralOS/network-root.git",
        )
        .expect("validated local Git root request");
        let resolved = resolve_git_package_source(
            &request,
            cache.join("git-sources"),
            LocalSourceLimits::default(),
        )
        .expect("resolve root for exact request validation");
        assert!(git_root_request_matches(
            &request,
            resolved.source(),
            resolved.key().source_lineage()
        ));
        let wrong_revision = GitSourceRequest::for_local_test_repository_with_lineage(
            &repository,
            Some("different-revision".to_owned()),
            "https://github.com/CathedralOS/network-root.git",
        )
        .expect("alternate revision request");
        assert!(!git_root_request_matches(
            &wrong_revision,
            resolved.source(),
            resolved.key().source_lineage()
        ));
        let wrong_locator = GitSourceRequest::for_local_test_repository_with_lineage(
            &repository,
            None,
            "https://github.com/CathedralOS/other-root.git",
        )
        .expect("alternate locator request");
        assert!(!git_root_request_matches(
            &wrong_locator,
            resolved.source(),
            resolved.key().source_lineage()
        ));

        let closure = resolve_git_package_closure(
            &request,
            &cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .expect("resolve repository-root Git closure");

        let root_binding = closure.source_requests().root();
        let PackageRootSourceRequest::Git(retained) = root_binding.request() else {
            panic!("Git adapter retains its root request")
        };
        assert_eq!(
            retained.requested_locator(),
            "https://github.com/CathedralOS/network-root.git"
        );
        assert_eq!(retained.requested_revision(), "HEAD");
        assert_eq!(retained.transport_profile(), GitTransportProfile::TestFile);
        assert_eq!(
            root_binding.selected().key().name().as_str(),
            "network-root"
        );
        assert!(closure.source_requests().dependencies().next().is_none());

        let _ = std::fs::remove_dir_all(repository);
        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn git_update_escalating_to_process_authority_blocks_and_requests_source_audit() {
        let repository = temp_root("git-process-authority-repository");
        let baseline_cache = temp_root("git-process-authority-baseline-cache");
        let candidate_cache = temp_root("git-process-authority-candidate-cache");
        let compiler_workspace = temp_root("git-process-authority-compiler-workspace");
        let process_fixture = fixture_root().join("process-exit");
        std::fs::create_dir_all(&repository).expect("create process-authority repository");
        std::fs::copy(
            process_fixture.join("build.omg"),
            repository.join("build.omg"),
        )
        .expect("copy stable package declaration");
        std::fs::write(
            repository.join("main.omg"),
            r#"use omega::language::std::console;

pub machine terminate(console: Console, return_code: i32)
{
}
"#,
        )
        .expect("write inert baseline package");
        run_test_git(&repository, ["init", "--quiet"]);
        run_test_git(
            &repository,
            ["config", "user.email", "omega@example.invalid"],
        );
        run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
        run_test_git(&repository, ["add", "."]);
        run_test_git(
            &repository,
            ["commit", "--quiet", "-m", "inert process boundary"],
        );
        let baseline_revision = test_git_head(&repository);

        std::fs::copy(
            process_fixture.join("main.omg"),
            repository.join("main.omg"),
        )
        .expect("copy canonical process-authority candidate");
        run_test_git(&repository, ["add", "main.omg"]);
        run_test_git(
            &repository,
            ["commit", "--quiet", "-m", "exercise process authority"],
        );
        let candidate_revision = test_git_head(&repository);
        assert_ne!(baseline_revision, candidate_revision);

        let canonical_lineage = "https://github.com/CathedralOS/process-exit.git";
        let baseline_request = GitSourceRequest::for_local_test_repository_with_lineage(
            &repository,
            Some(baseline_revision.clone()),
            canonical_lineage,
        )
        .expect("construct exact baseline Git request");
        let candidate_request = GitSourceRequest::for_local_test_repository_with_lineage(
            &repository,
            Some(candidate_revision.clone()),
            canonical_lineage,
        )
        .expect("construct exact candidate Git request");
        let baseline_sources = resolve_git_package_closure(
            &baseline_request,
            &baseline_cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .expect("resolve baseline Git custody");
        let candidate_sources = resolve_git_package_closure(
            &candidate_request,
            &candidate_cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .expect("resolve candidate Git custody");

        assert_eq!(
            baseline_sources.graph().root(),
            candidate_sources.graph().root(),
            "declared package identity and canonical Git lineage stay stable"
        );
        let baseline_custody = baseline_sources
            .custody(baseline_sources.graph().root())
            .expect("baseline root custody");
        let candidate_custody = candidate_sources
            .custody(candidate_sources.graph().root())
            .expect("candidate root custody");
        assert_ne!(
            baseline_custody.resolution(),
            candidate_custody.resolution()
        );
        assert_ne!(
            baseline_custody.snapshot_root(),
            candidate_custody.snapshot_root()
        );
        for (closure, expected_revision) in [
            (&baseline_sources, baseline_revision.as_str()),
            (&candidate_sources, candidate_revision.as_str()),
        ] {
            let PackageRootSourceRequest::Git(request) = closure.source_requests().root().request()
            else {
                panic!("authority update root must retain its exact Git request")
            };
            assert_eq!(request.requested_locator(), canonical_lineage);
            assert_eq!(request.requested_revision(), expected_revision);
            assert_eq!(request.transport_profile(), GitTransportProfile::TestFile);
            assert!(closure.source_requests().dependencies().next().is_none());
        }

        let baseline_reviews =
            compile_resolved_package_reviews(&baseline_sources, "windows_x64", &compiler_workspace)
                .expect("compile baseline package evidence");
        let candidate_reviews = compile_resolved_package_reviews(
            &candidate_sources,
            "windows_x64",
            &compiler_workspace,
        )
        .expect("compile candidate package evidence");
        let baseline = baseline_reviews
            .review(baseline_sources.graph().root())
            .expect("baseline root review");
        let candidate = candidate_reviews
            .review(candidate_sources.graph().root())
            .expect("candidate root review");
        assert!(baseline.projection().dangerous_authorities().is_empty());
        let [authority] = candidate.projection().dangerous_authorities() else {
            panic!("candidate must derive one effective dangerous authority")
        };
        assert_eq!(
            authority.class(),
            PackageReviewDangerousAuthorityClass::Process
        );
        assert_eq!(authority.service().path(), "Console");
        assert!(matches!(
            authority.service().owner(),
            PackageReviewNominalOwner::ToolchainSource(_)
        ));

        let conflicts = compare_review_only_capabilities(
            &baseline_reviews,
            &candidate_reviews,
            &candidate_sources,
            ReviewOnlyCapabilityConflictLimits::default(),
        )
        .expect("compare compiler-derived authority escalation");
        let [package] = conflicts.packages() else {
            panic!("authority escalation must affect exactly one package")
        };
        assert_eq!(package.conflicts().len(), 2);
        let dangerous = package
            .conflicts()
            .iter()
            .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::DangerousAuthority)
            .expect("effective authority must produce its own canonical conflict");
        assert_eq!(
            dangerous.change(),
            ReviewOnlyCapabilityConflictChange::Added
        );
        assert_eq!(dangerous.risk(), PackageReviewCanonicalRowRisk::Blocking);
        assert!(dangerous.is_blocking());
        assert!(dangerous.baseline_row().is_none());
        assert!(dangerous.baseline_source().is_none());
        assert!(dangerous.candidate_row().is_some());
        let dangerous_locations = dangerous
            .candidate_source()
            .and_then(|source| source.authored_locations())
            .expect("dangerous authority retains compiler-owned source coordinates");
        assert!(dangerous_locations.iter().any(|location| {
            location.role() == PackageReviewSourceLocationRole::AuthorityDeclaration
        }));
        assert!(dangerous_locations.iter().any(|location| {
            location.role() == PackageReviewSourceLocationRole::AuthorityExposure
                && location.relative_path() == "main.omg"
        }));

        let callable = package
            .conflicts()
            .iter()
            .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::Callable)
            .expect("changed declared and realized reach must change the callable row");
        assert_eq!(
            callable.change(),
            ReviewOnlyCapabilityConflictChange::Changed
        );
        assert_eq!(callable.risk(), PackageReviewCanonicalRowRisk::Blocking);
        assert!(callable.is_blocking());
        for source in [
            callable
                .baseline_source()
                .expect("baseline callable source"),
            callable
                .candidate_source()
                .expect("candidate callable source"),
        ] {
            assert!(
                source
                    .authored_locations()
                    .expect("callable source locations")
                    .iter()
                    .any(|location| {
                        location.role() == PackageReviewSourceLocationRole::Declaration
                            && location.relative_path() == "main.omg"
                    })
            );
        }

        let triage = triage_review_update(&baseline_reviews, &candidate_reviews, &BTreeSet::new());
        let [decision] = triage.decisions() else {
            panic!("authority escalation must produce one package decision")
        };
        assert_eq!(
            decision.disposition(),
            PackageTriageDisposition::BlockedCapabilityChange
        );
        assert_eq!(
            decision.reasons(),
            [
                PackageTriageReason::CapabilityOrApiChanged,
                PackageTriageReason::SourceChanged,
                PackageTriageReason::RetainedDangerousAuthority(
                    PackageReviewDangerousAuthorityClass::Process,
                ),
            ]
        );

        let review = assemble_update_source_review(
            &baseline_reviews,
            &candidate_reviews,
            baseline_sources.custodies(),
            &candidate_sources,
            PackageSourceReviewLimits::default(),
        )
        .expect("assemble exact source review for blocked escalation");
        let [patch] = review.source_patches() else {
            panic!("blocked authority escalation must carry one source patch")
        };
        assert_eq!(patch.baseline_key(), Some(baseline.key()));
        assert_eq!(patch.candidate_key(), candidate.key());
        assert_eq!(patch.changed_entries(), 1);
        assert!(!patch.requires_standalone_audit());
        assert!(patch.as_str().contains("mode update\n"));
        assert!(patch.as_str().contains("entry main.omg\n"));
        assert!(patch.as_str().contains("added lf reaches Console"));
        assert!(patch.as_str().contains("added lf invokes console;"));
        assert!(
            patch
                .as_str()
                .contains("added lf     console.exit_process(return_code);")
        );

        let _ = std::fs::remove_dir_all(repository);
        let _ = std::fs::remove_dir_all(baseline_cache);
        let _ = std::fs::remove_dir_all(candidate_cache);
        let _ = std::fs::remove_dir_all(compiler_workspace);
    }

    #[test]
    fn contextual_workspace_escape_becomes_external_local_lineage() {
        let sources = temp_root("contextual-workspace-sources");
        let workspace = sources.join("workspace");
        let root = workspace.join("packages/root");
        let external = sources.join("external");
        let cache = temp_root("contextual-workspace-cache");
        write_package(&root, "root-package", Some("../../../external"));
        write_package(&external, "external-package", None);
        let source_context = ExternalSourceContext::derive(b"workspace-consuming-lock");

        let closure = resolve_workspace_package_closure_in_context(
            &fixture_lineage(),
            WorkspaceMemberPath::parse("packages/root").expect("root member"),
            &workspace,
            source_context.clone(),
            &cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .expect("explicit context should route the workspace escape");

        assert_eq!(closure.graph().packages().len(), 2);
        let external = closure
            .custodies()
            .iter()
            .find(|custody| custody.key().name().as_str() == "external-package")
            .expect("external dependency custody");
        assert!(matches!(
            external.key().source_lineage(),
            SourceLineage::ExternalLocal(lineage)
                if lineage.source_context() == &source_context
        ));

        write_package(&root, "root-package", Some("../../../external/"));
        let malformed = resolve_workspace_package_closure_in_context(
            &fixture_lineage(),
            WorkspaceMemberPath::parse("packages/root").expect("root member"),
            &workspace,
            source_context,
            &cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .expect_err("a malformed workspace spelling must not switch source lanes");
        assert!(matches!(
            malformed,
            ResolveWorkspacePackageClosureError::Closure(
                PackageSourceClosureResolutionError::Adapter {
                    error: ResolveDependencySourceError::InvalidPath { .. },
                    ..
                }
            )
        ));

        let _ = std::fs::remove_dir_all(sources);
        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn rejects_workspace_escape_before_resolving_the_target() {
        let workspace = temp_root("escape-workspace");
        let package = workspace.join("packages/root");
        let cache = temp_root("escape-cache");
        std::fs::create_dir_all(&package).expect("create package");
        std::fs::write(
            package.join("build.omg"),
            r#"
            machine build(builder: &mut Build) {
                builder.package("root-package");
                builder.depend(Source::Path { location: "../../../outside" });
            }
            "#,
        )
        .expect("write build file");
        std::fs::write(package.join("main.omg"), "machine root() {}\n").expect("write source");

        let error = resolve_workspace_package_closure(
            &fixture_lineage(),
            WorkspaceMemberPath::parse("packages/root").expect("root member"),
            &workspace,
            &cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .expect_err("escaping dependency must reject");

        assert!(matches!(
            error,
            ResolveWorkspacePackageClosureError::Closure(
                PackageSourceClosureResolutionError::Adapter {
                    error: ResolveDependencySourceError::InvalidPath { .. },
                    ..
                }
            )
        ));

        let _ = std::fs::remove_dir_all(workspace);
        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn propagates_closure_resource_ceilings() {
        let cache = temp_root("limit-cache");
        let error = resolve_workspace_package_closure(
            &fixture_lineage(),
            WorkspaceMemberPath::parse("graph-workbench").expect("root member"),
            fixture_root(),
            &cache,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits {
                max_packages: 1,
                max_dependency_requests: 8,
                max_depth: 8,
            },
        )
        .expect_err("package ceiling must reject");

        assert!(matches!(
            error,
            ResolveWorkspacePackageClosureError::Closure(
                PackageSourceClosureResolutionError::LimitExceeded {
                    kind: PackageSourceClosureLimitKind::Packages,
                    ..
                }
            )
        ));

        let _ = std::fs::remove_dir_all(cache);
    }
}
