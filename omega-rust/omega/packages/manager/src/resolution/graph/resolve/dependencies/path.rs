use super::super::cache::{
    GitAcquisitionCache, SourceCacheLane, resolve_workspace_member_from_cache,
};
use super::super::errors::ResolveDependencySourceError;
use super::context::{WorkspaceContext, WorkspaceContextKind};
use super::external_local::{
    resolve_external_dependency, resolve_external_dependency_from_root, workspace_requester_root,
};
use crate::declarations::PackageKey;
use crate::resolution::source::workspace_path::source_relative_path;
use crate::resolution::source::{
    GitPackageSourceRequest, PackageSourceCustody, PackageSourceNavigation,
    ResolvePackageSourceError,
};
use omega_build_declarations::WorkspaceMemberPath;
use omega_package_source::{
    ExternalSourceContext, LocalSourceLimits, SourceLineage, WorkspaceLineageIdentity,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_path_dependency(
    requester: &PackageSourceCustody,
    location: &str,
    workspaces: &mut BTreeMap<WorkspaceLineageIdentity, WorkspaceContext>,
    external_roots: &mut BTreeMap<PackageKey, PathBuf>,
    external_context: Option<&ExternalSourceContext>,
    workspace_cache: SourceCacheLane<'_>,
    git_cache: SourceCacheLane<'_>,
    external_local_cache: SourceCacheLane<'_>,
    source_limits: LocalSourceLimits,
    git_acquisitions: &mut GitAcquisitionCache,
) -> Result<PackageSourceCustody, ResolveDependencySourceError> {
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
        Ok(member_path) => match &context.kind {
            WorkspaceContextKind::Git(repository) => {
                if !repository.declared_members.contains(&member_path) {
                    return Err(ResolveDependencySourceError::UndeclaredGitWorkspaceMember {
                        package: requester.key().clone(),
                        member_path,
                    });
                }
                let selection = super::git::workspace_member_selection(repository, &member_path);
                let package_name = crate::declarations::PackageName::parse(
                    selection.plan().selected_member().package_name().as_str(),
                )
                .expect("source and build package names share one grammar");
                let request = GitPackageSourceRequest::new(
                    repository.request.clone(),
                    crate::declarations::dependencies::read::PackageSelection::Named(package_name),
                );
                let resolved = git_acquisitions.resolve_selected(
                    &request,
                    git_cache,
                    workspace_cache,
                    repository.source_limits,
                )?;
                let source_path = source_relative_path(&member_path);
                if resolved.navigation() != &PackageSourceNavigation::Member(source_path)
                    || resolved.resolution() != &repository.resolution
                {
                    return Err(ResolveDependencySourceError::ConflictingWorkspaceRoot {
                        identity: workspace_identity,
                    });
                }
                Ok(resolved.into_custody())
            }
            WorkspaceContextKind::Local { root, .. } => resolve_workspace_member_from_cache(
                &context.root_source,
                source_relative_path(&member_path),
                root,
                workspace_cache,
                source_limits,
            )
            .map(|resolved| resolved.into_custody())
            .map_err(ResolveDependencySourceError::from),
        },
        Err(_)
            if matches!(
                &context.kind,
                WorkspaceContextKind::Local {
                    allows_external_paths: true,
                    ..
                }
            ) && external_context.is_some()
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

pub(in super::super) fn normalize_member_path(
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
    WorkspaceMemberPath::parse(components.join("/"))
        .map_err(|error| invalid_path(location, &error.to_string()))
}

pub(in super::super) fn workspace_path_escapes(
    requester_member: Option<&str>,
    location: &str,
) -> bool {
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

pub(super) fn invalid_path(location: &str, reason: &str) -> ResolveDependencySourceError {
    ResolveDependencySourceError::InvalidPath {
        location: location.to_owned(),
        reason: reason.to_owned(),
    }
}
