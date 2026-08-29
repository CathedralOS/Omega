//! Resolve dependency rows after a root source has entered custody.

use super::super::reconciliation::{
    PackageRootSourceRequest, PackageSourceClosureLimits, PackageSourceClosureResolutionError,
    ResolvedPackageSourceClosure, resolve_package_source_closure_with_limits,
};
use super::cache::{
    GitAcquisitionCache, SourceCacheLane, resolve_external_local_package_from_cache,
    resolve_workspace_member_from_cache,
};
use super::errors::ResolveDependencySourceError;
use crate::manifest::dependencies::read::DependencySourceRequest;
use crate::resolution::binding::git_selection::{GitWorkspaceEvidence, GitWorkspaceSelectionPlan};
use crate::resolution::binding::{
    GitPackageSourceRequest, PackageSourceCustody, PackageSourceNavigation,
    PackageSourceSelectionEvidence, ResolvePackageSourceError, bind_git_member_package_custody,
};
use omega_package_source::LocalSourceLimits;
use omega_package_source::{
    ExternalSourceContext, ImmutableSourceResolution, PackageKey, SourceLineage,
    WorkspaceLineageIdentity, WorkspaceMemberPath,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkspaceContext {
    pub(super) root_source: SourceLineage,
    pub(super) root: PathBuf,
    pub(super) allows_external_paths: bool,
    git_repository: Option<GitRepositoryContext>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitRepositoryContext {
    resolution: ImmutableSourceResolution,
    declared_members: BTreeSet<WorkspaceMemberPath>,
    workspace_evidence: Option<GitWorkspaceEvidence>,
    source_limits: LocalSourceLimits,
}

impl WorkspaceContext {
    pub(super) fn local(
        root_source: SourceLineage,
        root: PathBuf,
        allows_external_paths: bool,
    ) -> Self {
        Self {
            root_source,
            root,
            allows_external_paths,
            git_repository: None,
        }
    }
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
    git_acquisitions: &mut GitAcquisitionCache,
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
                selection,
                ..
            } => {
                let request = GitPackageSourceRequest::new(
                    omega_package_source::GitSourceRequest::new(
                        repository.clone(),
                        Some(revision.clone()),
                    )?,
                    selection.clone(),
                );
                let resolved =
                    git_acquisitions.resolve_selected(&request, git_cache, source_limits)?;
                register_git_repository(
                    workspaces,
                    resolved.key().source_lineage(),
                    resolved.acquisition_root(),
                    resolved.resolution(),
                    resolved.selection_evidence(),
                    resolved.source_limits(),
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
                    Ok(member_path) => {
                        if let Some(git_repository) = &context.git_repository {
                            if !git_repository.declared_members.contains(&member_path) {
                                return Err(
                                    ResolveDependencySourceError::UndeclaredGitWorkspaceMember {
                                        package: requester.key().clone(),
                                        member_path,
                                    },
                                );
                            }
                            bind_git_member_package_custody(
                                requester.key().source_lineage().clone(),
                                git_repository.resolution.clone(),
                                &context.root,
                                member_path.clone(),
                                git_workspace_member_selection(git_repository, &member_path),
                                git_repository.source_limits,
                            )
                            .map_err(ResolveDependencySourceError::from)
                        } else {
                            resolve_workspace_member_from_cache(
                                &context.root_source,
                                member_path,
                                &context.root,
                                workspace_cache,
                                source_limits,
                            )
                            .map(|resolved| resolved.into_custody())
                            .map_err(ResolveDependencySourceError::from)
                        }
                    }
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

pub(super) fn register_git_repository(
    workspaces: &mut BTreeMap<WorkspaceLineageIdentity, WorkspaceContext>,
    root_source: &SourceLineage,
    acquisition_root: &Path,
    resolution: &ImmutableSourceResolution,
    selection_evidence: &PackageSourceSelectionEvidence,
    source_limits: LocalSourceLimits,
) -> Result<WorkspaceLineageIdentity, ResolveDependencySourceError> {
    let (declared_members, workspace_evidence) = match selection_evidence {
        PackageSourceSelectionEvidence::Root => (BTreeSet::new(), None),
        PackageSourceSelectionEvidence::GitWorkspace(plan) => (
            plan.members()
                .iter()
                .map(|member| WorkspaceMemberPath::from(member.member_path().clone()))
                .collect(),
            Some(plan.workspace_evidence().clone()),
        ),
    };
    let identity = WorkspaceLineageIdentity::from_root_source(root_source)
        .map_err(ResolvePackageSourceError::from)?;
    let context = WorkspaceContext {
        root_source: root_source.clone(),
        root: acquisition_root.to_path_buf(),
        allows_external_paths: false,
        git_repository: Some(GitRepositoryContext {
            resolution: resolution.clone(),
            declared_members,
            workspace_evidence,
            source_limits: source_limits.compiler_bounded(),
        }),
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

fn git_workspace_member_selection(
    repository: &GitRepositoryContext,
    member_path: &WorkspaceMemberPath,
) -> GitWorkspaceSelectionPlan {
    let shared_path = omega_build_declarations::WorkspaceMemberPath::parse(member_path.as_str())
        .expect("package-source and build-declaration member paths share one grammar");
    repository
        .workspace_evidence
        .as_ref()
        .and_then(|evidence| evidence.select_declared_member(&shared_path))
        .expect("declared Git member set and retained workspace selection are one custody value")
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
        lineage @ (SourceLineage::GitHub(_) | SourceLineage::GitLab(_) | SourceLineage::Git(_)) => {
            let identity = WorkspaceLineageIdentity::from_root_source(lineage)
                .map_err(ResolvePackageSourceError::from)?;
            if !workspaces.contains_key(&identity) {
                return Err(ResolveDependencySourceError::UnknownWorkspace {
                    package: requester.key().clone(),
                });
            }
            let member = match requester.navigation() {
                PackageSourceNavigation::Root => None,
                PackageSourceNavigation::Member(path) => Some(path.as_str().to_owned()),
            };
            Ok((identity, member))
        }
        SourceLineage::ExternalLocal(_) => Err(ResolveDependencySourceError::UnknownWorkspace {
            package: requester.key().clone(),
        }),
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
