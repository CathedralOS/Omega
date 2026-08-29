//! Adapt authenticated declaration bytes to Omega workspace selection semantics.

use super::{
    GitWorkspaceMemberBuild, GitWorkspaceSelectionError, discover_git_workspace,
    plan_git_workspace_selection,
};
use crate::identity::PackageName;
use crate::resolution::source::workspace_path::{
    authored_workspace_member_path, source_relative_path,
};
use crate::resolution::source::{GitWorkspaceSelectionDeclarations, GitWorkspaceSelectionEvidence};
use omega_package_source::{
    GitWorkspaceDeclaration, GitWorkspaceProjectionPlanner, GitWorkspaceSelection,
};

pub(in crate::resolution::source::git) struct ManagerGitWorkspacePlanner<'a> {
    selected: &'a PackageName,
}

impl<'a> ManagerGitWorkspacePlanner<'a> {
    pub(in crate::resolution::source::git) fn new(selected: &'a PackageName) -> Self {
        Self { selected }
    }
}

impl GitWorkspaceProjectionPlanner for ManagerGitWorkspacePlanner<'_> {
    type Error = GitWorkspaceSelectionError;
    type Evidence = GitWorkspaceSelectionEvidence;

    fn discover_members(
        &mut self,
        root_declaration: &GitWorkspaceDeclaration,
    ) -> Result<Vec<omega_package_source::SourceRelativePath>, Self::Error> {
        let discovery = discover_git_workspace(root_declaration.bytes())?;
        Ok(discovery
            .member_paths()
            .iter()
            .map(source_relative_path)
            .collect())
    }

    fn select_member(
        &mut self,
        root_declaration: &GitWorkspaceDeclaration,
        member_declarations: &[GitWorkspaceDeclaration],
    ) -> Result<GitWorkspaceSelection<Self::Evidence>, Self::Error> {
        let declarations = member_declarations
            .iter()
            .map(|declaration| {
                let member_path = declaration
                    .member_path()
                    .expect("member declaration has one member path");
                let member_path = authored_workspace_member_path(member_path);
                (member_path, declaration.bytes().to_vec())
            })
            .collect::<Vec<_>>();
        let supplied = declarations
            .iter()
            .map(|(member_path, bytes)| GitWorkspaceMemberBuild::new(member_path, bytes.as_slice()))
            .collect::<Vec<_>>();
        let selected = omega_build_declarations::ProjectName::parse(self.selected.as_str())
            .expect("source and build declaration package names share one grammar");
        let plan = plan_git_workspace_selection(&selected, root_declaration.bytes(), &supplied)?;
        let selected_member = source_relative_path(plan.selected_member_path());
        Ok(GitWorkspaceSelection::new(
            selected_member,
            GitWorkspaceSelectionEvidence::new(
                plan,
                GitWorkspaceSelectionDeclarations::new(
                    root_declaration.bytes().to_vec(),
                    declarations,
                ),
            ),
        ))
    }
}
