use super::projection::project_package_build;
use super::{ResolvePackageSourceError, ResolvedPackageSource};
use crate::resolution::identity::{
    ImmutableSourceResolution, PackageKey, SourceContentDigest, SourceLineage,
    WorkspaceLineageIdentity, WorkspaceMemberLineage, WorkspaceMemberPath,
};
use crate::source::{LocalSourceLimits, ResolvedLocalSnapshot, resolve_local_source_snapshot};
use std::path::{Path, PathBuf};

/// Snapshot one workspace member and bind it to the workspace root's source
/// lineage plus its normalized member-relative path.
///
/// The live member is derived only as `live_workspace_root/member_path`; the
/// caller does not supply a second spelling to reconcile. It must remain a
/// strict descendant of the canonical workspace root. Only that member is
/// passed to local snapshot custody.
pub fn resolve_workspace_member_package_source(
    workspace_root_source: &SourceLineage,
    member_path: WorkspaceMemberPath,
    live_workspace_root: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    let limits = limits.compiler_bounded();
    let workspace_identity = WorkspaceLineageIdentity::from_root_source(workspace_root_source)?;
    let requested_workspace_root = live_workspace_root.as_ref();
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
    let source = resolve_local_source_snapshot(&canonical_declared_member_root, cache_dir, limits)?;
    let lineage =
        SourceLineage::Workspace(WorkspaceMemberLineage::new(workspace_identity, member_path));
    let (declaration, dependency_requests) = project_package_build(&source.snapshot_root, false)?;
    let resolution = ImmutableSourceResolution::workspace(SourceContentDigest::derive(
        source.normalized.content_identity.as_bytes(),
    ));

    Ok(ResolvedPackageSource::from_resolved_parts(
        PackageKey::new(declaration.name, lineage),
        resolution,
        source.snapshot_root.clone(),
        limits,
        dependency_requests,
        source,
    ))
}

fn canonical_workspace_path(path: &Path) -> Result<PathBuf, ResolvePackageSourceError> {
    std::fs::canonicalize(path).map_err(|error| ResolvePackageSourceError::WorkspacePath {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}
