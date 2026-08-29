use super::{BuildDeclarationCommitment, GitWorkspaceSelectionError};
use omega_build_declarations::BuildDeclarationKind;
use omega_build_declarations::{ProjectName, WorkspaceMemberPath};

/// Authenticated declaration bytes associated with one declared member path.
///
/// Bytes are borrowed only for planning and are never retained in the plan.
#[derive(Debug, Clone, Copy)]
pub struct GitWorkspaceMemberBuild<'a> {
    member_path: &'a WorkspaceMemberPath,
    build_bytes: &'a [u8],
}

/// First-stage discovery of the exact declarations a caller must obtain.
///
/// The workspace declaration is committed here so the second-stage selection
/// can prove it planned against the same authenticated root bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorkspaceDiscovery {
    workspace_declaration: BuildDeclarationEvidence,
    member_paths: Vec<WorkspaceMemberPath>,
}

impl GitWorkspaceDiscovery {
    pub(super) fn new(
        workspace_declaration: BuildDeclarationEvidence,
        member_paths: Vec<WorkspaceMemberPath>,
    ) -> Self {
        Self {
            workspace_declaration,
            member_paths,
        }
    }

    pub const fn workspace_declaration(&self) -> &BuildDeclarationEvidence {
        &self.workspace_declaration
    }

    pub fn member_paths(&self) -> &[WorkspaceMemberPath] {
        &self.member_paths
    }
}

impl<'a> GitWorkspaceMemberBuild<'a> {
    pub const fn new(member_path: &'a WorkspaceMemberPath, build_bytes: &'a [u8]) -> Self {
        Self {
            member_path,
            build_bytes,
        }
    }

    pub const fn member_path(&self) -> &'a WorkspaceMemberPath {
        self.member_path
    }

    pub const fn build_bytes(&self) -> &'a [u8] {
        self.build_bytes
    }
}

/// Replay evidence for one declaration, without retaining package-authored
/// source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildDeclarationEvidence {
    repository_path: String,
    byte_count: usize,
    commitment: BuildDeclarationCommitment,
}

impl BuildDeclarationEvidence {
    pub(super) fn from_bytes(repository_path: String, bytes: &[u8]) -> Self {
        Self {
            repository_path,
            byte_count: bytes.len(),
            commitment: BuildDeclarationCommitment::derive(bytes),
        }
    }

    pub fn repository_path(&self) -> &str {
        &self.repository_path
    }

    pub const fn byte_count(&self) -> usize {
        self.byte_count
    }

    pub const fn commitment(&self) -> &BuildDeclarationCommitment {
        &self.commitment
    }
}

/// One declared workspace member after role and static dependency validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorkspaceMemberPlan {
    member_path: WorkspaceMemberPath,
    package_name: ProjectName,
    role: BuildDeclarationKind,
    declaration: BuildDeclarationEvidence,
}

impl GitWorkspaceMemberPlan {
    pub(super) fn new(
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

    pub const fn role(&self) -> BuildDeclarationKind {
        self.role
    }

    pub const fn declaration(&self) -> &BuildDeclarationEvidence {
        &self.declaration
    }
}

/// Complete declaration evidence for one Git workspace, independent of which
/// declared member a request selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorkspaceEvidence {
    workspace_declaration: BuildDeclarationEvidence,
    members: Vec<GitWorkspaceMemberPlan>,
}

impl GitWorkspaceEvidence {
    pub(super) fn new(
        workspace_declaration: BuildDeclarationEvidence,
        members: Vec<GitWorkspaceMemberPlan>,
    ) -> Self {
        Self {
            workspace_declaration,
            members,
        }
    }

    pub const fn workspace_declaration(&self) -> &BuildDeclarationEvidence {
        &self.workspace_declaration
    }

    pub fn members(&self) -> &[GitWorkspaceMemberPlan] {
        &self.members
    }

    pub fn select_declared_member(
        &self,
        member_path: &WorkspaceMemberPath,
    ) -> Option<GitWorkspaceSelectionPlan> {
        self.members
            .iter()
            .any(|member| member.member_path() == member_path)
            .then(|| GitWorkspaceSelectionPlan {
                selected_member_path: member_path.clone(),
                workspace: self.clone(),
            })
    }
}

/// Deterministic package selection over complete workspace evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorkspaceSelectionPlan {
    selected_member_path: WorkspaceMemberPath,
    workspace: GitWorkspaceEvidence,
}

impl GitWorkspaceSelectionPlan {
    pub(super) fn new(
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

    pub const fn workspace_evidence(&self) -> &GitWorkspaceEvidence {
        &self.workspace
    }

    pub const fn workspace_declaration(&self) -> &BuildDeclarationEvidence {
        self.workspace.workspace_declaration()
    }

    pub fn members(&self) -> &[GitWorkspaceMemberPlan] {
        self.workspace.members()
    }

    pub fn for_declared_member(&self, member_path: &WorkspaceMemberPath) -> Option<Self> {
        self.workspace.select_declared_member(member_path)
    }

    /// Replay the complete selection and require byte-for-byte declaration
    /// evidence and semantics to remain unchanged.
    pub fn replay(
        &self,
        root_build_bytes: &[u8],
        member_builds: &[GitWorkspaceMemberBuild<'_>],
    ) -> Result<(), GitWorkspaceSelectionError> {
        let replayed = super::plan_git_workspace_selection(
            self.selected_member().package_name(),
            root_build_bytes,
            member_builds,
        )?;
        if &replayed != self {
            return Err(GitWorkspaceSelectionError::DeclarationEvidenceChanged);
        }
        Ok(())
    }
}
