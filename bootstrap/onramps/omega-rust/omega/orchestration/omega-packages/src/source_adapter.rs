use crate::closure_resolution::{
    PackageSourceClosureLimits, PackageSourceClosureResolutionError, PackageSourceCustody,
    ResolvedPackageSourceClosure, resolve_package_source_closure_with_limits,
};
use crate::dependency_projection::DependencySourceRequest;
use crate::identity::{
    ExternalSourceContext, PackageKey, SourceLineage, WorkspaceLineageIdentity, WorkspaceMemberPath,
};
use crate::package_source::{
    ResolvePackageSourceError, resolve_external_local_package_source, resolve_git_package_source,
    resolve_workspace_member_package_source,
};
use crate::source::{GitSourceRequest, GitSourceRequestError, LocalSourceLimits};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ResolveWorkspacePackageClosureError {
    Root(ResolvePackageSourceError),
    Closure(PackageSourceClosureResolutionError<ResolveDependencySourceError>),
}

#[derive(Debug)]
pub enum ResolveExternalLocalPackageClosureError {
    Root(ResolvePackageSourceError),
    Closure(PackageSourceClosureResolutionError<ResolveDependencySourceError>),
}

impl fmt::Display for ResolveExternalLocalPackageClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(error) => write!(formatter, "cannot resolve root package: {error}"),
            Self::Closure(error) => write!(formatter, "cannot resolve package closure: {error}"),
        }
    }
}

impl std::error::Error for ResolveExternalLocalPackageClosureError {}

impl fmt::Display for ResolveWorkspacePackageClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(error) => write!(formatter, "cannot resolve root package: {error}"),
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
    let workspace_cache = cache_dir.join("workspace-members");
    let git_cache = cache_dir.join("git-sources");
    let workspace_identity = WorkspaceLineageIdentity::from_root_source(workspace_root_source)
        .map_err(ResolvePackageSourceError::from)
        .map_err(ResolveWorkspacePackageClosureError::Root)?;
    let root = resolve_workspace_member_package_source(
        workspace_root_source,
        root_member_path,
        live_workspace_root,
        &workspace_cache,
        source_limits,
    )
    .map_err(ResolveWorkspacePackageClosureError::Root)?
    .into_custody();

    let canonical_workspace_root = live_workspace_root.canonicalize().map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::WorkspacePath {
            path: live_workspace_root.to_path_buf(),
            message: error.to_string(),
        })
    })?;
    let mut workspaces = BTreeMap::from([(
        workspace_identity,
        WorkspaceContext {
            root_source: workspace_root_source.clone(),
            root: canonical_workspace_root,
            allows_external_paths: true,
        },
    )]);

    resolve_registered_package_closure(
        root,
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
    let cache_dir = cache_dir.as_ref();
    let local_cache = cache_dir.join("external-local-sources");
    let workspace_cache = cache_dir.join("workspace-members");
    let git_cache = cache_dir.join("git-sources");
    let root = resolve_external_local_package_source(
        live_root,
        &local_cache,
        source_limits,
        source_context.clone(),
    )
    .map_err(ResolveExternalLocalPackageClosureError::Root)?;
    let mut external_roots = BTreeMap::from([(
        root.key().clone(),
        root.source().canonical_live_root.clone(),
    )]);

    resolve_registered_package_closure(
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
    resolve_package_source_closure_with_limits(root, closure_limits, |requester, request| {
        match request {
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
        }
    })
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
    use crate::PackageSourceClosureLimitKind;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../../fixtures/packages")
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
                "const PACKAGE: Package = Package {{ name: \"{name}\" }};\n\nmachine build(builder: &mut Build) {{\n{dependency}}}\n"
            ),
        )
        .expect("write build file");
        std::fs::write(root.join("main.omg"), "machine root() {}\n").expect("write source");
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
            const PACKAGE: Package = Package { name: "root-package" };
            machine build(builder: &mut Build) {
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
