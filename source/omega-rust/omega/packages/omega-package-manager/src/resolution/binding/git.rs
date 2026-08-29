use super::projection::project_package_build;
use super::{
    GitWorkspaceSelectionDeclarations, GitWorkspaceSelectionEvidence, ResolvePackageSourceError,
    ResolvedPackageSource,
};
use crate::manifest::dependencies::read::PackageSelection;
use crate::resolution::binding::git_selection::{
    GitWorkspaceMemberBuild, MAX_BUILD_DECLARATION_BYTES, MAX_TOTAL_BUILD_DECLARATION_BYTES,
    MAX_WORKSPACE_MEMBERS, discover_git_workspace, plan_git_workspace_selection,
};
use omega_package_source::{
    GitAcquisitionPin, GitSourceRequest, LocalSourceLimits, ResolvedGitSource,
    SourceResolverStorage,
};
use omega_package_source::{
    GitCommitId, GitTreeId, ImmutableSourceResolution, PackageKey, SourceLineage,
};
use omega_package_source::{
    GitWorkspaceDeclaration, GitWorkspaceDeclarationLimits, GitWorkspaceProjectionError,
    GitWorkspaceProjectionPlanner, GitWorkspaceSelection, RetainedStorageLane,
    resolve_git_source_in_lane, resolve_git_workspace_member_from_pin_in_lanes,
};

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
    cache_dir: impl AsRef<std::path::Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir)?;
    resolve_git_package_source_with_storage(request, &storage, limits)
}

fn resolve_git_root_package_source_in_lane(
    request: &GitSourceRequest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let limits = limits.compiler_bounded();
    let lineage = request.lineage().clone();
    let source = resolve_git_source_in_lane(request, lane, limits)?;
    bind_git_root_package_source(lineage, source, limits)
}

pub(crate) fn resolve_selected_git_package_source_in_lanes(
    request: &GitPackageSourceRequest,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    resolve_selected_git_package_source_from_pin_in_lanes(
        request,
        None,
        git_lane,
        member_lane,
        limits,
    )
}

pub(crate) fn resolve_selected_git_package_source_from_pin_in_lanes(
    request: &GitPackageSourceRequest,
    pin: Option<&GitAcquisitionPin>,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let limits = limits.compiler_bounded();
    match request.selection() {
        PackageSelection::Root => {
            resolve_git_root_package_source_in_lane(request.acquisition(), git_lane, limits)
        }
        PackageSelection::Named(package) => {
            let mut planner = ManagerGitWorkspacePlanner::new(package);
            let projected = resolve_git_workspace_member_from_pin_in_lanes(
                request.acquisition(),
                pin,
                git_lane,
                member_lane,
                limits,
                GitWorkspaceDeclarationLimits::new(
                    MAX_WORKSPACE_MEMBERS,
                    u64::try_from(MAX_BUILD_DECLARATION_BYTES)
                        .expect("declaration limit fits canonical u64"),
                    u64::try_from(MAX_TOTAL_BUILD_DECLARATION_BYTES)
                        .expect("declaration aggregate limit fits canonical u64"),
                ),
                &mut planner,
            )
            .map_err(|error| match error {
                GitWorkspaceProjectionError::Source(error) => {
                    ResolvePackageSourceError::Source(error)
                }
                GitWorkspaceProjectionError::Planner(error) => {
                    ResolvePackageSourceError::GitWorkspaceSelection(error)
                }
            })?;
            let (source, evidence) = projected.into_parts();
            bind_projected_git_package_source(
                request.acquisition().lineage().clone(),
                source,
                limits,
                evidence,
            )
        }
    }
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
    let result = resolve_selected_git_package_source_in_lanes(
        request,
        storage.git_sources(),
        storage.workspace_members(),
        limits,
    );
    storage.verify_path_identity()?;
    result
}

struct ManagerGitWorkspacePlanner<'a> {
    selected: &'a omega_package_source::PackageName,
}

impl<'a> ManagerGitWorkspacePlanner<'a> {
    fn new(selected: &'a omega_package_source::PackageName) -> Self {
        Self { selected }
    }
}

impl GitWorkspaceProjectionPlanner for ManagerGitWorkspacePlanner<'_> {
    type Error = crate::resolution::binding::git_selection::GitWorkspaceSelectionError;
    type Evidence = GitWorkspaceSelectionEvidence;

    fn discover_members(
        &mut self,
        root_declaration: &GitWorkspaceDeclaration,
    ) -> Result<Vec<omega_package_source::WorkspaceMemberPath>, Self::Error> {
        let discovery = discover_git_workspace(root_declaration.bytes())?;
        Ok(discovery
            .member_paths()
            .iter()
            .cloned()
            .map(omega_package_source::WorkspaceMemberPath::from)
            .collect())
    }

    fn select_member(
        &mut self,
        root_declaration: &GitWorkspaceDeclaration,
        member_declarations: &[GitWorkspaceDeclaration],
    ) -> Result<GitWorkspaceSelection<Self::Evidence>, Self::Error> {
        let declarations = member_declarations
            .iter()
            .map(|declaration| {
                let member_path = declaration
                    .member_path()
                    .expect("member declaration has one member path");
                let member_path =
                    omega_build_declarations::WorkspaceMemberPath::parse(member_path.as_str())
                        .expect("source and build declaration paths share one grammar");
                (member_path, declaration.bytes().to_vec())
            })
            .collect::<Vec<_>>();
        let supplied = declarations
            .iter()
            .map(|(member_path, bytes)| GitWorkspaceMemberBuild::new(member_path, bytes.as_slice()))
            .collect::<Vec<_>>();
        let selected = omega_build_declarations::ProjectName::parse(self.selected.as_str())
            .expect("source and build declaration package names share one grammar");
        let plan = plan_git_workspace_selection(&selected, root_declaration.bytes(), &supplied)?;
        let selected_member =
            omega_package_source::WorkspaceMemberPath::from(plan.selected_member_path().clone());
        Ok(GitWorkspaceSelection::new(
            selected_member,
            GitWorkspaceSelectionEvidence::new(
                plan,
                GitWorkspaceSelectionDeclarations::new(
                    root_declaration.bytes().to_vec(),
                    declarations,
                ),
            ),
        ))
    }
}

fn bind_projected_git_package_source(
    lineage: SourceLineage,
    source: ResolvedGitSource,
    limits: LocalSourceLimits,
    selection_evidence: GitWorkspaceSelectionEvidence,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let projection = source.workspace_projection().ok_or_else(|| {
        ResolvePackageSourceError::GitWorkspaceMemberNavigation {
            member_path: omega_package_source::WorkspaceMemberPath::from(
                selection_evidence.plan().selected_member_path().clone(),
            ),
            message: "selective source result omitted workspace projection custody".to_owned(),
        }
    })?;
    let selected_member_path = omega_package_source::WorkspaceMemberPath::from(
        selection_evidence.plan().selected_member_path().clone(),
    );
    if projection.selected_member_path() != &selected_member_path
        || projection.selected_member_tree() != source.materialized_tree()
    {
        return Err(ResolvePackageSourceError::GitWorkspaceMemberNavigation {
            member_path: selected_member_path,
            message: "source and manager workspace selection evidence disagree".to_owned(),
        });
    }
    let snapshot_root = source.snapshot_root().to_path_buf();
    let (declaration, dependency_requests) = project_package_build(&snapshot_root, false)?;
    let resolution = ImmutableSourceResolution::git(
        GitCommitId::parse_hex(source.commit())?,
        GitTreeId::parse_hex(source.tree())?,
    )?;
    let materialization = super::PackageSourceMaterialization::from_local(source.local());
    selection_evidence.revalidate().map_err(|error| {
        ResolvePackageSourceError::GitWorkspaceMemberNavigation {
            member_path: projection.selected_member_path().clone(),
            message: error.to_string(),
        }
    })?;

    Ok(ResolvedPackageSource::from_resolved_parts(
        PackageKey::new(declaration.name, lineage),
        resolution,
        materialization,
        snapshot_root,
        super::PackageSourceNavigation::Member(projection.selected_member_path().clone()),
        super::PackageSourceSelectionEvidence::GitWorkspace(selection_evidence),
        limits,
        dependency_requests,
        source,
    ))
}

fn bind_git_root_package_source(
    lineage: SourceLineage,
    source: ResolvedGitSource,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let snapshot_root = source.snapshot_root().to_path_buf();
    let (declaration, dependency_requests) = project_package_build(&snapshot_root, false)?;
    let resolution = ImmutableSourceResolution::git(
        GitCommitId::parse_hex(source.commit())?,
        GitTreeId::parse_hex(source.tree())?,
    )?;
    let materialization = super::PackageSourceMaterialization::from_local(source.local());

    Ok(ResolvedPackageSource::from_resolved_parts(
        PackageKey::new(declaration.name, lineage),
        resolution,
        materialization,
        snapshot_root,
        super::PackageSourceNavigation::Root,
        super::PackageSourceSelectionEvidence::Root,
        limits,
        dependency_requests,
        source,
    ))
}
