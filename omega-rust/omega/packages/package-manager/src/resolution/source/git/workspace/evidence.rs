//! Replayable authenticated declarations for one selected workspace member.

use super::{GitWorkspaceMemberBuild, GitWorkspaceSelectionPlan, account_declaration_bytes};
use crate::resolution::source::PackageSourceSelectionEvidenceError;
use build_declarations::WorkspaceMemberPath;

/// Raw authenticated declarations retained separately from the selected
/// member's compilation root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitWorkspaceSelectionDeclarations {
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
pub(crate) struct GitWorkspaceSelectionEvidence {
    plan: GitWorkspaceSelectionPlan,
    declarations: GitWorkspaceSelectionDeclarations,
    executed: Option<(
        compiler::SourceBuildDeclaration,
        Vec<compiler::SourceBuildDeclaration>,
    )>,
}

impl GitWorkspaceSelectionEvidence {
    pub fn new(
        plan: GitWorkspaceSelectionPlan,
        declarations: GitWorkspaceSelectionDeclarations,
    ) -> Self {
        Self {
            plan,
            declarations,
            executed: None,
        }
    }

    pub(crate) fn from_executed(
        selected: &build_declarations::ProjectName,
        root: compiler::SourceBuildDeclaration,
        members: Vec<compiler::SourceBuildDeclaration>,
    ) -> Result<Self, String> {
        let plan = super::executed_selection_plan(selected, &root, &members)?;
        let root_bytes = root
            .source_capture()
            .tree()
            .read_to_string(std::path::Path::new("build.omg"))
            .map_err(|error| error.to_string())?
            .as_bytes()
            .to_vec();
        let declarations = plan
            .members()
            .iter()
            .zip(&members)
            .map(|(member, observed)| {
                let bytes = observed
                    .source_capture()
                    .tree()
                    .read_to_string(std::path::Path::new("build.omg"))
                    .map_err(|error| error.to_string())?;
                Ok((member.member_path().clone(), bytes.as_bytes().to_vec()))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            plan,
            declarations: GitWorkspaceSelectionDeclarations::new(root_bytes, declarations),
            executed: Some((root, members)),
        })
    }

    pub const fn plan(&self) -> &GitWorkspaceSelectionPlan {
        &self.plan
    }

    pub const fn declarations(&self) -> &GitWorkspaceSelectionDeclarations {
        &self.declarations
    }

    pub fn revalidate(&self) -> Result<(), PackageSourceSelectionEvidenceError> {
        if let Some((root, members)) = &self.executed {
            let replayed = super::executed_selection_plan(
                self.plan().selected_member().package_name(),
                root,
                members,
            )
            .map_err(PackageSourceSelectionEvidenceError::Executed)?;
            return if replayed == self.plan {
                Ok(())
            } else {
                Err(PackageSourceSelectionEvidenceError::Selection(
                    super::GitWorkspaceSelectionError::DeclarationEvidenceChanged,
                ))
            };
        }
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
