use crate::sources::GitWorkspaceSelectionDeclarations;
use crate::sources::git::workspace::GitWorkspaceEvidence;
use omega_build_declarations::WorkspaceMemberPath;
use omega_package_source::{
    GitSourceRequest, ImmutableSourceResolution, LocalSourceLimits, SourceLineage,
};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in super::super) struct WorkspaceContext {
    pub(in super::super) root_source: SourceLineage,
    pub(super) kind: WorkspaceContextKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum WorkspaceContextKind {
    Local {
        root: PathBuf,
        allows_external_paths: bool,
    },
    Git(GitRepositoryContext),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GitRepositoryContext {
    pub(super) request: GitSourceRequest,
    pub(super) resolution: ImmutableSourceResolution,
    pub(super) declared_members: BTreeSet<WorkspaceMemberPath>,
    pub(super) workspace_evidence: Option<GitRepositoryWorkspaceEvidence>,
    pub(super) source_limits: LocalSourceLimits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GitRepositoryWorkspaceEvidence {
    pub(super) workspace: GitWorkspaceEvidence,
    pub(super) declarations: GitWorkspaceSelectionDeclarations,
}

impl WorkspaceContext {
    pub(in super::super) fn local(
        root_source: SourceLineage,
        root: PathBuf,
        allows_external_paths: bool,
    ) -> Self {
        Self {
            root_source,
            kind: WorkspaceContextKind::Local {
                root,
                allows_external_paths,
            },
        }
    }
}
