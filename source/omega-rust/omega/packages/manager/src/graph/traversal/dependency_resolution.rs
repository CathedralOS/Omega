//! Resolve dependency rows after a root source has entered custody.

use super::super::reconciliation::{
    PackageRootSourceRequest, PackageSourceClosureLimits, PackageSourceClosureResolutionError,
    ResolvedPackageSourceClosure, resolve_package_source_closure_with_limits,
};
use super::cache::{
    SourceCacheLane, resolve_external_local_package_from_cache, resolve_git_from_cache,
    resolve_workspace_member_from_cache,
};
use super::errors::ResolveDependencySourceError;
use crate::manifest::dependency_projection::DependencySourceRequest;
use crate::package::{PackageSourceCustody, ResolvePackageSourceError};
use crate::source::identity::{
    ExternalSourceContext, PackageKey, SourceLineage, WorkspaceLineageIdentity, WorkspaceMemberPath,
};
use crate::source::{GitSourceRequest, LocalSourceLimits};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkspaceContext {
    pub(super) root_source: SourceLineage,
    pub(super) root: PathBuf,
    pub(super) allows_external_paths: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_registered_package_closure(
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

pub(super) fn register_workspace(
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
        resolved.source().canonical_live_root(),
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
