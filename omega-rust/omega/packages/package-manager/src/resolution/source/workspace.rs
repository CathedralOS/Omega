use super::projection::project_package_build;
use super::{ResolvePackageSourceError, ResolvedPackageSource, SourceRootRole};
use package_source::local::operations::resolve_local_source_snapshot_in_lane;
use package_source::storage::RetainedStorageLane;
use package_source::{
    ImmutableSourceResolution, SourceContentDigest, SourceLineage, SourceRelativePath,
    WorkspaceLineageIdentity, WorkspaceMemberLineage,
};
use package_source::{LocalSourceLimits, ResolvedLocalSnapshot};
use std::path::{Path, PathBuf};

/// Resolve one declared workspace member in a retained lane for the given root
/// role: a member selected as a compilation root may declare an application,
/// a dependency member may not.
pub(crate) fn resolve_workspace_member_source_in_lane(
    workspace_root_source: &SourceLineage,
    member_path: SourceRelativePath,
    live_workspace_root: impl AsRef<Path>,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    role: SourceRootRole,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    let limits = limits.compiler_bounded();
    let workspace_identity = WorkspaceLineageIdentity::from_root_source(workspace_root_source)?;
    let canonical_declared_member_root =
        validate_workspace_member_root(live_workspace_root.as_ref(), &member_path)?;
    let source =
        resolve_local_source_snapshot_in_lane(&canonical_declared_member_root, lane, limits)?;
    bind_workspace_member_source(workspace_identity, member_path, source, limits, role)
}

fn validate_workspace_member_root(
    requested_workspace_root: &Path,
    member_path: &SourceRelativePath,
) -> Result<PathBuf, ResolvePackageSourceError> {
    let declared_member_root = requested_workspace_root.join(member_path.as_str());

    let canonical_workspace_root = canonical_workspace_path(requested_workspace_root)?;
    let canonical_declared_member_root = canonical_workspace_path(&declared_member_root)?;

    if canonical_declared_member_root == canonical_workspace_root {
        return Err(ResolvePackageSourceError::WorkspaceMemberIsRoot {
            workspace_root: canonical_workspace_root,
        });
    }
    if !canonical_declared_member_root.starts_with(&canonical_workspace_root) {
        return Err(ResolvePackageSourceError::WorkspaceMemberEscapesRoot {
            workspace_root: canonical_workspace_root,
            member_root: canonical_declared_member_root,
        });
    }
    Ok(canonical_declared_member_root)
}

fn bind_workspace_member_source(
    workspace_identity: WorkspaceLineageIdentity,
    member_path: SourceRelativePath,
    source: ResolvedLocalSnapshot,
    limits: LocalSourceLimits,
    role: SourceRootRole,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    let lineage =
        SourceLineage::Workspace(WorkspaceMemberLineage::new(workspace_identity, member_path));
    let declaration = project_package_build(source.snapshot_root(), role)?;
    let resolution = ImmutableSourceResolution::workspace(SourceContentDigest::derive(
        source.normalized().content_identity.as_bytes(),
    ));
    super::resolved::resolve_local_snapshot_source(source, declaration, lineage, resolution, limits)
}

fn canonical_workspace_path(path: &Path) -> Result<PathBuf, ResolvePackageSourceError> {
    std::fs::canonicalize(path).map_err(|error| ResolvePackageSourceError::WorkspacePath {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}
