use super::projection::project_package_build;
use super::selection::read_bounded_declaration;
use super::{PackageSourceCustody, ResolvePackageSourceError, ResolvedPackageSource};
use crate::manifest::PackageDeclarationError;
use crate::manifest::dependencies::read::PackageSelection;
use crate::resolution::binding::git_selection::{
    GitWorkspaceMemberBuild, GitWorkspaceSelectionPlan, account_declaration_bytes,
    discover_git_workspace, plan_git_workspace_selection,
};
use omega_package_source::{
    GitCommitId, GitTreeId, ImmutableSourceResolution, PackageKey, SourceContentDigest,
    SourceLineage,
};
use omega_package_source::{
    GitSourceRequest, LocalSourceLimits, ResolvedGitSource, SourceResolverStorage,
};
use omega_package_source::{RetainedStorageLane, resolve_git_source_in_lane, resolve_local_source};
use std::path::{Path, PathBuf};

/// One exact package selection over one validated Git acquisition request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitPackageSourceRequest {
    acquisition: GitSourceRequest,
    selection: PackageSelection,
}

impl GitPackageSourceRequest {
    pub fn new(acquisition: GitSourceRequest, selection: PackageSelection) -> Self {
        Self {
            acquisition,
            selection,
        }
    }

    pub fn root(acquisition: GitSourceRequest) -> Self {
        Self::new(acquisition, PackageSelection::Root)
    }

    pub const fn acquisition(&self) -> &GitSourceRequest {
        &self.acquisition
    }

    pub const fn selection(&self) -> &PackageSelection {
        &self.selection
    }

    pub fn requested_locator(&self) -> &str {
        self.acquisition.requested_locator()
    }

    pub fn requested_revision(&self) -> &str {
        self.acquisition.requested_revision()
    }

    pub fn transport_profile(&self) -> omega_package_source::GitTransportProfile {
        self.acquisition.transport_profile()
    }
}

/// Resolve a network Git request, then derive package identity only from the
/// canonical request lineage and the package declaration in the immutable
/// snapshot.
#[cfg(test)]
pub fn resolve_git_package_source(
    request: &GitSourceRequest,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir)?;
    resolve_git_package_source_with_storage(request, &storage, limits)
}

fn resolve_selected_git_package_source_in_lane(
    request: &GitPackageSourceRequest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let limits = limits.compiler_bounded();
    let lineage = request.acquisition().lineage().clone();
    let source = resolve_git_source_in_lane(request.acquisition(), lane, limits)?;
    bind_git_package_source(lineage, source, limits, request.selection())
}

pub fn resolve_git_package_source_with_storage(
    request: &GitSourceRequest,
    storage: &SourceResolverStorage,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    resolve_selected_git_package_source_with_storage(
        &GitPackageSourceRequest::root(request.clone()),
        storage,
        limits,
    )
}

pub fn resolve_selected_git_package_source_with_storage(
    request: &GitPackageSourceRequest,
    storage: &SourceResolverStorage,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    storage.verify_path_identity()?;
    let result =
        resolve_selected_git_package_source_in_lane(request, storage.git_sources(), limits);
    storage.verify_path_identity()?;
    result
}

pub(crate) fn bind_git_package_source(
    lineage: SourceLineage,
    source: ResolvedGitSource,
    limits: LocalSourceLimits,
    selection: &PackageSelection,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let acquisition_root = source.snapshot_root().to_path_buf();
    let (snapshot_root, navigation, selection_evidence) = match selection {
        PackageSelection::Root => (
            acquisition_root.clone(),
            super::PackageSourceNavigation::Root,
            super::PackageSourceSelectionEvidence::Root,
        ),
        PackageSelection::Named(package) => {
            let (member_path, plan) = select_named_git_member(&acquisition_root, package)?;
            let member_root = validate_git_member_root(&acquisition_root, &member_path)?;
            (
                member_root,
                super::PackageSourceNavigation::Member(member_path),
                super::PackageSourceSelectionEvidence::GitWorkspace(plan),
            )
        }
    };
    let (declaration, dependency_requests) = project_package_build(&snapshot_root, false)?;
    let resolution = ImmutableSourceResolution::git(
        GitCommitId::parse_hex(source.commit())?,
        GitTreeId::parse_hex(source.tree())?,
        SourceContentDigest::derive(source.local().content_identity.as_bytes()),
    )?;
    let acquisition_materialization =
        super::PackageSourceMaterialization::from_local(source.local());
    let materialization = if snapshot_root == acquisition_root {
        acquisition_materialization.clone()
    } else {
        super::PackageSourceMaterialization::from_local(&resolve_local_source(
            &snapshot_root,
            limits,
        )?)
    };

    Ok(ResolvedPackageSource::from_resolved_parts(
        PackageKey::new(declaration.name, lineage),
        resolution,
        acquisition_materialization,
        materialization,
        acquisition_root,
        snapshot_root,
        navigation,
        selection_evidence,
        limits,
        dependency_requests,
        source,
    ))
}

fn select_named_git_member(
    acquisition_root: &Path,
    selected: &omega_package_source::PackageName,
) -> Result<
    (
        omega_package_source::WorkspaceMemberPath,
        GitWorkspaceSelectionPlan,
    ),
    ResolvePackageSourceError,
> {
    let root_build = read_git_build(acquisition_root, Path::new("build.omg"))?;
    let discovery = discover_git_workspace(&root_build)?;
    let mut total_bytes = account_declaration_bytes(0, &root_build)?;
    let mut member_builds = Vec::with_capacity(discovery.member_paths().len());
    for declared_path in discovery.member_paths() {
        let member_path = omega_package_source::WorkspaceMemberPath::from(declared_path.clone());
        validate_git_member_root(acquisition_root, &member_path)?;
        let build_path = PathBuf::from(declared_path.as_str()).join("build.omg");
        let build = read_git_build(acquisition_root, &build_path)?;
        total_bytes = account_declaration_bytes(total_bytes, &build)?;
        member_builds.push((declared_path.clone(), build));
    }
    let supplied = member_builds
        .iter()
        .map(|(member_path, build_bytes)| {
            GitWorkspaceMemberBuild::new(member_path, build_bytes.as_slice())
        })
        .collect::<Vec<_>>();
    let selected = omega_build_declarations::ProjectName::parse(selected.as_str())
        .expect("package-source and build-declaration names share one grammar");
    let plan = plan_git_workspace_selection(&selected, &root_build, &supplied)?;
    let member_path =
        omega_package_source::WorkspaceMemberPath::from(plan.selected_member_path().clone());
    Ok((member_path, plan))
}

fn read_git_build(
    acquisition_root: &Path,
    relative_path: &Path,
) -> Result<Vec<u8>, ResolvePackageSourceError> {
    let path = acquisition_root.join(relative_path);
    read_bounded_declaration(&path).map_err(|error| {
        ResolvePackageSourceError::Declaration(if error.kind() == std::io::ErrorKind::NotFound {
            PackageDeclarationError::MissingBuildFile { path }
        } else {
            PackageDeclarationError::ReadBuildFile {
                path,
                message: error.to_string(),
            }
        })
    })
}

fn validate_git_member_root(
    acquisition_root: &Path,
    member_path: &omega_package_source::WorkspaceMemberPath,
) -> Result<PathBuf, ResolvePackageSourceError> {
    let mut current = acquisition_root.to_path_buf();
    for component in member_path.as_str().split('/') {
        current.push(component);
        let metadata = std::fs::symlink_metadata(&current).map_err(|error| {
            ResolvePackageSourceError::GitWorkspaceMemberNavigation {
                member_path: member_path.clone(),
                message: error.to_string(),
            }
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ResolvePackageSourceError::GitWorkspaceMemberNavigation {
                member_path: member_path.clone(),
                message: "member navigation contains a symbolic link".to_owned(),
            });
        }
    }
    if !current.is_dir() {
        return Err(ResolvePackageSourceError::GitWorkspaceMemberNavigation {
            member_path: member_path.clone(),
            message: "member root is not a directory".to_owned(),
        });
    }
    Ok(current)
}

pub(crate) fn bind_git_member_package_custody(
    lineage: SourceLineage,
    resolution: ImmutableSourceResolution,
    acquisition_root: &Path,
    acquisition_materialization: super::PackageSourceMaterialization,
    member_path: omega_package_source::WorkspaceMemberPath,
    selection_plan: GitWorkspaceSelectionPlan,
    limits: LocalSourceLimits,
) -> Result<PackageSourceCustody, ResolvePackageSourceError> {
    let snapshot_root = validate_git_member_root(acquisition_root, &member_path)?;
    let (declaration, dependency_requests) = project_package_build(&snapshot_root, false)?;
    let materialization = super::PackageSourceMaterialization::from_local(&resolve_local_source(
        &snapshot_root,
        limits,
    )?);
    Ok(PackageSourceCustody::from_resolved_parts(
        PackageKey::new(declaration.name, lineage),
        resolution,
        acquisition_materialization,
        materialization,
        acquisition_root.to_path_buf(),
        snapshot_root,
        super::PackageSourceNavigation::Member(member_path),
        super::PackageSourceSelectionEvidence::GitWorkspace(selection_plan),
        limits,
        dependency_requests,
    ))
}
