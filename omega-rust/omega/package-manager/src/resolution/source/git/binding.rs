//! Bind one resolved Git snapshot to its declared package identity and custody.

use super::super::projection::project_package_build;
use super::super::{
    GitWorkspaceSelectionEvidence, PackageSourceMaterialization, PackageSourceNavigation,
    PackageSourceSelectionEvidence, ResolvePackageSourceError, ResolvedPackageSource,
};
use crate::declarations::PackageKey;
use crate::resolution::source::workspace_path::source_relative_path;
use package_source::{GitCommitId, GitTreeId, ImmutableSourceResolution, ResolvedGitSource};

pub(super) fn bind_projected_git_package_source(
    source: ResolvedGitSource,
    selection_evidence: GitWorkspaceSelectionEvidence,
    application_root_allowed: bool,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let lineage = source.lineage().clone();
    let limits = source.source_limits();
    let projection = source.workspace_projection().ok_or_else(|| {
        ResolvePackageSourceError::GitWorkspaceMemberNavigation {
            member_path: source_relative_path(selection_evidence.plan().selected_member_path()),
            message: "selective source result omitted workspace projection custody".to_owned(),
        }
    })?;
    let selected_member_path =
        source_relative_path(selection_evidence.plan().selected_member_path());
    if projection.selected_member_path() != &selected_member_path
        || projection.selected_member_tree() != source.materialized_tree()
    {
        return Err(ResolvePackageSourceError::GitWorkspaceMemberNavigation {
            member_path: selected_member_path,
            message: "source and manager workspace selection evidence disagree".to_owned(),
        });
    }
    let snapshot_root = source.snapshot_root().to_path_buf();
    let declaration = project_package_build(&snapshot_root, application_root_allowed)?;
    let resolution = ImmutableSourceResolution::git(
        GitCommitId::parse_hex(source.commit())?,
        GitTreeId::parse_hex(source.tree())?,
    )?;
    let materialization = PackageSourceMaterialization::from_local(source.local());
    selection_evidence.revalidate().map_err(|error| {
        ResolvePackageSourceError::GitWorkspaceMemberNavigation {
            member_path: projection.selected_member_path().clone(),
            message: error.to_string(),
        }
    })?;

    Ok(ResolvedPackageSource::from_resolved_parts(
        PackageKey::new(declaration.name, lineage),
        declaration.role,
        resolution,
        materialization,
        snapshot_root,
        PackageSourceNavigation::Member(projection.selected_member_path().clone()),
        PackageSourceSelectionEvidence::GitWorkspace(selection_evidence),
        limits,
        declaration.dependencies,
        source,
    ))
}

pub(super) fn bind_git_root_package_source(
    source: ResolvedGitSource,
    application_root_allowed: bool,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    let lineage = source.lineage().clone();
    let limits = source.source_limits();
    let snapshot_root = source.snapshot_root().to_path_buf();
    let declaration = project_package_build(&snapshot_root, application_root_allowed)?;
    let resolution = ImmutableSourceResolution::git(
        GitCommitId::parse_hex(source.commit())?,
        GitTreeId::parse_hex(source.tree())?,
    )?;
    let materialization = PackageSourceMaterialization::from_local(source.local());

    Ok(ResolvedPackageSource::from_resolved_parts(
        PackageKey::new(declaration.name, lineage),
        declaration.role,
        resolution,
        materialization,
        snapshot_root,
        PackageSourceNavigation::Root,
        PackageSourceSelectionEvidence::Root,
        limits,
        declaration.dependencies,
        source,
    ))
}
