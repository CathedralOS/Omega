//! Replayable authenticated declarations for one selected workspace member.

use super::{GitWorkspaceMemberBuild, GitWorkspaceSelectionPlan, account_declaration_bytes};
use crate::sources::PackageSourceSelectionEvidenceError;
use omega_build_declarations::WorkspaceMemberPath;

/// Raw authenticated declarations retained separately from the selected
/// member's compilation root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorkspaceSelectionDeclarations {
    root: Vec<u8>,
    members: Vec<(WorkspaceMemberPath, Vec<u8>)>,
}

impl GitWorkspaceSelectionDeclarations {
    pub fn new(root: Vec<u8>, members: Vec<(WorkspaceMemberPath, Vec<u8>)>) -> Self {
        Self { root, members }
    }

    pub fn root(&self) -> &[u8] {
        &self.root
    }

    pub fn members(&self) -> &[(WorkspaceMemberPath, Vec<u8>)] {
        &self.members
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorkspaceSelectionEvidence {
    plan: GitWorkspaceSelectionPlan,
    declarations: GitWorkspaceSelectionDeclarations,
}

impl GitWorkspaceSelectionEvidence {
    pub fn new(
        plan: GitWorkspaceSelectionPlan,
        declarations: GitWorkspaceSelectionDeclarations,
    ) -> Self {
        Self { plan, declarations }
    }

    pub const fn plan(&self) -> &GitWorkspaceSelectionPlan {
        &self.plan
    }

    pub const fn declarations(&self) -> &GitWorkspaceSelectionDeclarations {
        &self.declarations
    }

    pub fn for_declared_member(&self, member_path: &WorkspaceMemberPath) -> Option<Self> {
        self.plan
            .for_declared_member(member_path)
            .map(|plan| Self::new(plan, self.declarations.clone()))
    }

    pub fn revalidate(&self) -> Result<(), PackageSourceSelectionEvidenceError> {
        let root_build = self.declarations().root();
        let mut total_bytes = account_declaration_bytes(0, root_build)
            .map_err(PackageSourceSelectionEvidenceError::Selection)?;
        let mut member_builds = Vec::with_capacity(self.plan().members().len());
        for (member_path, bytes) in self.declarations().members() {
            total_bytes = account_declaration_bytes(total_bytes, bytes)
                .map_err(PackageSourceSelectionEvidenceError::Selection)?;
            member_builds.push((member_path.clone(), bytes.as_slice()));
        }
        let supplied = member_builds
            .iter()
            .map(|(member_path, bytes)| GitWorkspaceMemberBuild::new(member_path, bytes))
            .collect::<Vec<_>>();
        self.plan()
            .replay(root_build, &supplied)
            .map_err(PackageSourceSelectionEvidenceError::Selection)
    }
}
