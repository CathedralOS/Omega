use crate::closure_resolution::{
    PackageSourceClosureLimits, PackageSourceClosureResolutionError, PackageSourceCustody,
    ResolvedPackageSourceClosure, resolve_package_source_closure_with_limits,
};
use crate::dependency_projection::DependencySourceRequest;
use crate::identity::{SourceLineage, WorkspaceLineageIdentity, WorkspaceMemberPath};
use crate::package_source::{
    ResolvePackageSourceError, resolve_git_package_source, resolve_workspace_member_package_source,
};
use crate::source::{GitSourceSpec, LocalSourceLimits};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ResolveWorkspacePackageClosureError {
    Root(ResolvePackageSourceError),
    Closure(PackageSourceClosureResolutionError<ResolveDependencySourceError>),
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceContext {
    root_source: SourceLineage,
    root: PathBuf,
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
    let cache_dir = cache_dir.as_ref();
    let workspace_cache = cache_dir.join("workspace-members");
    let git_cache = cache_dir.join("git-sources");
    let workspace_identity = WorkspaceLineageIdentity::from_root_source(workspace_root_source)
        .map_err(ResolvePackageSourceError::from)
        .map_err(ResolveWorkspacePackageClosureError::Root)?;
    let root = resolve_workspace_member_package_source(
        workspace_root_source,
        root_member_path,
        live_workspace_root.as_ref(),
        &workspace_cache,
        source_limits,
    )
    .map_err(ResolveWorkspacePackageClosureError::Root)?
    .into_custody();

    let canonical_workspace_root =
        live_workspace_root
            .as_ref()
            .canonicalize()
            .map_err(|error| {
                ResolveWorkspacePackageClosureError::Root(
                    ResolvePackageSourceError::WorkspacePath {
                        path: live_workspace_root.as_ref().to_path_buf(),
                        message: error.to_string(),
                    },
                )
            })?;
    let mut workspaces = BTreeMap::from([(
        workspace_identity,
        WorkspaceContext {
            root_source: workspace_root_source.clone(),
            root: canonical_workspace_root,
        },
    )]);

    resolve_package_source_closure_with_limits(root, closure_limits, |requester, request| {
        match request {
            DependencySourceRequest::Git {
                repository,
                revision,
                ..
            } => {
                let resolved = resolve_git_package_source(
                    &GitSourceSpec {
                        url: repository.clone(),
                        rev: Some(revision.clone()),
                    },
                    &git_cache,
                    source_limits,
                )?;
                register_workspace(
                    &mut workspaces,
                    resolved.key().source_lineage(),
                    resolved.snapshot_root(),
                )?;
                Ok(resolved.into_custody())
            }
            DependencySourceRequest::Path { location, .. } => {
                let (workspace_identity, base) = requester_workspace(requester, &mut workspaces)?;
                let member_path = normalize_member_path(base.as_deref(), location)?;
                let context = workspaces.get(&workspace_identity).ok_or_else(|| {
                    ResolveDependencySourceError::UnknownWorkspace {
                        package: requester.key().clone(),
                    }
                })?;
                resolve_workspace_member_package_source(
                    &context.root_source,
                    member_path,
                    &context.root,
                    &workspace_cache,
                    source_limits,
                )
                .map(|resolved| resolved.into_custody())
                .map_err(ResolveDependencySourceError::from)
            }
        }
    })
    .map_err(ResolveWorkspacePackageClosureError::Closure)
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
