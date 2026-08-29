use super::projection::project_package_build;
use super::{PackageSourceCustody, ResolvePackageSourceError, ResolvedPackageSource};
use crate::manifest::dependencies::read::PackageSelection;
use crate::manifest::{BuildDeclaration, extract_build_declaration};
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
    let (snapshot_root, navigation) = match selection {
        PackageSelection::Root => (
            acquisition_root.clone(),
            super::PackageSourceNavigation::Root,
        ),
        PackageSelection::Named(package) => {
            let member_path = select_named_git_member(&acquisition_root, package)?;
            let member_root = validate_git_member_root(&acquisition_root, &member_path)?;
            (
                member_root,
                super::PackageSourceNavigation::Member(member_path),
            )
        }
    };
    let (declaration, dependency_requests) = project_package_build(&snapshot_root, false)?;
    let resolution = ImmutableSourceResolution::git(
        GitCommitId::parse_hex(source.commit())?,
        GitTreeId::parse_hex(source.tree())?,
        SourceContentDigest::derive(source.local().content_identity.as_bytes()),
    )?;
    let materialization = if snapshot_root == acquisition_root {
        super::PackageSourceMaterialization::from_local(source.local())
    } else {
        super::PackageSourceMaterialization::from_local(&resolve_local_source(
            &snapshot_root,
            limits,
        )?)
    };

    Ok(ResolvedPackageSource::from_resolved_parts(
        PackageKey::new(declaration.name, lineage),
        resolution,
        materialization,
        acquisition_root,
        snapshot_root,
        navigation,
        limits,
        dependency_requests,
        source,
    ))
}

fn select_named_git_member(
    acquisition_root: &Path,
    selected: &omega_package_source::PackageName,
) -> Result<omega_package_source::WorkspaceMemberPath, ResolvePackageSourceError> {
    let declaration = extract_build_declaration(acquisition_root)?;
    let BuildDeclaration::Workspace(workspace) = declaration else {
        return Err(
            ResolvePackageSourceError::NamedGitSelectionRequiresWorkspace {
                found: declaration.kind(),
            },
        );
    };
    let mut matches = Vec::new();
    for member_path in workspace.members {
        let member_root = validate_git_member_root(acquisition_root, &member_path)?;
        let (declaration, _) = project_package_build(&member_root, false).map_err(|error| {
            ResolvePackageSourceError::GitWorkspaceMemberInvalid {
                member_path: member_path.clone(),
                error: Box::new(error),
            }
        })?;
        if &declaration.name == selected {
            matches.push(member_path);
        }
    }
    match matches.len() {
        0 => Err(ResolvePackageSourceError::NamedGitPackageMissing {
            package: selected.clone(),
        }),
        1 => Ok(matches.pop().expect("one selected Git member")),
        _ => Err(ResolvePackageSourceError::NamedGitPackageDuplicate {
            package: selected.clone(),
            member_paths: matches,
        }),
    }
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
    member_path: omega_package_source::WorkspaceMemberPath,
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
        materialization,
        acquisition_root.to_path_buf(),
        snapshot_root,
        super::PackageSourceNavigation::Member(member_path),
        limits,
        dependency_requests,
    ))
}
