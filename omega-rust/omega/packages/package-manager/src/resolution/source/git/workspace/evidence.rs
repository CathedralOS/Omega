//! Replayable authenticated declarations for one selected workspace member.

use super::GitWorkspaceSelectionPlan;
use crate::resolution::source::PackageSourceSelectionEvidenceError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitWorkspaceSelectionEvidence {
    plan: GitWorkspaceSelectionPlan,
    executed: (
        compiler::SourceBuildDeclaration,
        Vec<crate::resolution::source::WorkspaceMemberObservation>,
    ),
}

impl GitWorkspaceSelectionEvidence {
    pub(crate) fn from_executed(
        selected: &build_declarations::ProjectName,
        root: compiler::SourceBuildDeclaration,
        members: Vec<crate::resolution::source::WorkspaceMemberObservation>,
    ) -> Result<Self, String> {
        let declarations = members
            .iter()
            .map(|member| member.declaration().clone())
            .collect::<Vec<_>>();
        let plan = super::executed_selection_plan(selected, &root, &declarations)?;
        Ok(Self {
            plan,
            executed: (root, members),
        })
    }

    pub const fn plan(&self) -> &GitWorkspaceSelectionPlan {
        &self.plan
    }

    pub(crate) fn observations(&self) -> &[crate::resolution::source::WorkspaceMemberObservation] {
        &self.executed.1
    }

    pub fn revalidate(&self) -> Result<(), PackageSourceSelectionEvidenceError> {
        let (root, members) = &self.executed;
        let declarations = members
            .iter()
            .map(|member| member.declaration().clone())
            .collect::<Vec<_>>();
        let replayed = super::executed_selection_plan(
            self.plan().selected_member().package_name(),
            root,
            &declarations,
        )
        .map_err(PackageSourceSelectionEvidenceError::executed)?;
        if replayed == self.plan {
            Ok(())
        } else {
            Err(super::GitWorkspaceSelectionError::DeclarationEvidenceChanged.into())
        }
    }
}
