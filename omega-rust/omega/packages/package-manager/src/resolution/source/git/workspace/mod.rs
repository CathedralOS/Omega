//! Select one package from executed declarations bound to authenticated Git inputs.

mod commitment;
mod error;
mod evidence;
mod executed;
pub(crate) use executed::selection_plan as executed_selection_plan;
mod limits;

use build_declarations::{BuildDeclarationKind, ProjectName, WorkspaceMemberPath};
pub(crate) use commitment::BuildDeclarationCommitment;
pub(crate) use error::{GitWorkspaceSelectionError, GitWorkspaceSelectionLimit};
pub(crate) use evidence::GitWorkspaceSelectionEvidence;
pub(crate) use limits::account_declaration_bytes;
pub(crate) use limits::{
    MAX_BUILD_DECLARATION_BYTES, MAX_TOTAL_BUILD_DECLARATION_BYTES, MAX_WORKSPACE_MEMBERS,
};

pub(in crate::resolution::source) fn git_workspace_declaration_limits()
-> package_source::GitWorkspaceDeclarationLimits {
    package_source::GitWorkspaceDeclarationLimits::new(
        MAX_WORKSPACE_MEMBERS,
        u64::try_from(MAX_BUILD_DECLARATION_BYTES).expect("declaration limit fits canonical u64"),
        u64::try_from(MAX_TOTAL_BUILD_DECLARATION_BYTES)
            .expect("declaration aggregate limit fits canonical u64"),
    )
}

/// Replay evidence for one declaration, without retaining package-authored
/// source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildDeclarationEvidence {
    repository_path: String,
    byte_count: usize,
    commitment: BuildDeclarationCommitment,
}

impl BuildDeclarationEvidence {
    fn from_bytes(repository_path: String, bytes: &[u8]) -> Self {
        Self {
            repository_path,
            byte_count: bytes.len(),
            commitment: BuildDeclarationCommitment::derive(bytes),
        }
    }
}

/// One workspace member from an executed declaration and exact source capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitWorkspaceMemberPlan {
    member_path: WorkspaceMemberPath,
    package_name: ProjectName,
    role: BuildDeclarationKind,
    declaration: BuildDeclarationEvidence,
}

impl GitWorkspaceMemberPlan {
    fn new(
        member_path: WorkspaceMemberPath,
        package_name: ProjectName,
        role: BuildDeclarationKind,
        declaration: BuildDeclarationEvidence,
    ) -> Self {
        Self {
            member_path,
            package_name,
            role,
            declaration,
        }
    }

    pub const fn member_path(&self) -> &WorkspaceMemberPath {
        &self.member_path
    }

    pub const fn package_name(&self) -> &ProjectName {
        &self.package_name
    }
}

/// Complete declaration evidence for one Git workspace, independent of which
/// declared member a request selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitWorkspaceEvidence {
    workspace_declaration: BuildDeclarationEvidence,
    members: Vec<GitWorkspaceMemberPlan>,
}

impl GitWorkspaceEvidence {
    fn new(
        workspace_declaration: BuildDeclarationEvidence,
        members: Vec<GitWorkspaceMemberPlan>,
    ) -> Self {
        Self {
            workspace_declaration,
            members,
        }
    }

    pub fn members(&self) -> &[GitWorkspaceMemberPlan] {
        &self.members
    }
}

/// Deterministic package selection over complete workspace evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitWorkspaceSelectionPlan {
    selected_member_path: WorkspaceMemberPath,
    workspace: GitWorkspaceEvidence,
}

impl GitWorkspaceSelectionPlan {
    fn new(
        selected_member_path: WorkspaceMemberPath,
        workspace_declaration: BuildDeclarationEvidence,
        members: Vec<GitWorkspaceMemberPlan>,
    ) -> Self {
        Self {
            selected_member_path,
            workspace: GitWorkspaceEvidence::new(workspace_declaration, members),
        }
    }

    pub const fn selected_member_path(&self) -> &WorkspaceMemberPath {
        &self.selected_member_path
    }

    pub fn selected_member(&self) -> &GitWorkspaceMemberPlan {
        self.workspace
            .members()
            .iter()
            .find(|member| member.member_path() == self.selected_member_path())
            .expect("selection plan retains its selected declared member")
    }
}
