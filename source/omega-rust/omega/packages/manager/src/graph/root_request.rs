//! Exact request that selected a dependency graph root.

use omega_package_source::GitSourceRequest;
use omega_package_source::{ExternalSourceContext, SourceLineage, WorkspaceMemberPath};
use std::path::PathBuf;

/// Dependency requests belong to a requester package. The graph root has no
/// requester, so its exact source request is retained separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageRootSourceRequest {
    Git(GitSourceRequest),
    WorkspaceMember {
        workspace_root_source: SourceLineage,
        member_path: WorkspaceMemberPath,
        requested_workspace_root: PathBuf,
    },
    ExternalLocal {
        requested_root: PathBuf,
        source_context: ExternalSourceContext,
    },
}
