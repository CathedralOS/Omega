//! Bind a workspace catalog to declarations observed from exact acquired code.
//! This does not turn an unselected member's normal return into a completed
//! Build or include its dependencies in the selected member's closure.

use super::{BuildDeclarationEvidence, GitWorkspaceMemberPlan, GitWorkspaceSelectionPlan};
use build_declarations::{BuildDeclaration, ProjectName};
use compiler::SourceBuildDeclaration;
use std::path::Path;

pub(crate) fn selection_plan(
    selected: &ProjectName,
    root: &SourceBuildDeclaration,
    members: &[SourceBuildDeclaration],
) -> Result<GitWorkspaceSelectionPlan, String> {
    if !root.is_settled() {
        return Err("workspace catalog has not settled its output obligations".into());
    }
    let BuildDeclaration::Workspace(workspace) = root.declaration() else {
        return Err("named Git selection requires an executed workspace declaration".into());
    };
    if workspace.members.len() != members.len()
        || workspace.members.len() > super::MAX_WORKSPACE_MEMBERS
    {
        return Err("workspace declaration roster differs from its executed members".into());
    }
    let root_bytes = root
        .source_capture()
        .tree()
        .read_to_string(Path::new("build.omg"))
        .map_err(|error| error.to_string())?;
    let mut total = super::account_declaration_bytes(0, root_bytes.as_bytes())
        .map_err(|error| error.to_string())?;
    let mut plans = Vec::with_capacity(members.len());
    let mut selected_path = None;
    for (path, observed) in workspace.members.iter().zip(members) {
        if observed.source_capture().canonical_live_root()
            != root
                .source_capture()
                .canonical_live_root()
                .join(path.as_str())
        {
            return Err("workspace declaration came from a different member capture".into());
        }
        let bytes = observed
            .source_capture()
            .tree()
            .read_to_string(Path::new("build.omg"))
            .map_err(|error| error.to_string())?;
        total = super::account_declaration_bytes(total, bytes.as_bytes())
            .map_err(|error| error.to_string())?;
        let (name, role) = match observed.declaration() {
            BuildDeclaration::Package(package) => (&package.name, observed.declaration().kind()),
            BuildDeclaration::Application(application) => {
                (&application.name, observed.declaration().kind())
            }
            BuildDeclaration::Workspace(_) => {
                return Err("workspace members cannot recursively declare another catalog".into());
            }
        };
        if plans
            .iter()
            .any(|member: &GitWorkspaceMemberPlan| member.package_name() == name)
        {
            return Err(format!(
                "workspace repeats declared package name `{}`",
                name.as_str()
            ));
        }
        if name == selected {
            selected_path = Some(path.clone());
        }
        plans.push(GitWorkspaceMemberPlan::new(
            path.clone(),
            name.clone(),
            role,
            BuildDeclarationEvidence::from_bytes(
                format!("{}/build.omg", path.as_str()),
                bytes.as_bytes(),
            ),
        ));
    }
    let selected_path = selected_path
        .ok_or_else(|| format!("workspace has no declared package `{}`", selected.as_str()))?;
    Ok(GitWorkspaceSelectionPlan::new(
        selected_path,
        BuildDeclarationEvidence::from_bytes("build.omg".into(), root_bytes.as_bytes()),
        plans,
    ))
}
