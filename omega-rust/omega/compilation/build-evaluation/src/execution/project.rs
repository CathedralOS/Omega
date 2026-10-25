//! Normalize the role returned by execution, independent of source spelling.

use build_declarations::{
    ApplicationDeclaration, BuildDeclaration, BuildRequest, PackageDeclaration,
    WorkspaceDeclaration,
};

use crate::terminal_requests::IssuedBuildRequest;

pub(super) fn declaration(requests: &[IssuedBuildRequest]) -> Result<BuildDeclaration, String> {
    let mut role = None;
    let mut members = Vec::new();
    let mut artifact_only = false;
    for issued in requests {
        match issued.request() {
            BuildRequest::Package(name) => {
                if role.is_some() || !members.is_empty() {
                    return Err("Build returned duplicate or mixed project roles".into());
                }
                role = Some(BuildDeclaration::Package(PackageDeclaration {
                    name: name.clone(),
                }));
            }
            BuildRequest::Application(name) => {
                if role.is_some() || !members.is_empty() {
                    return Err("Build returned duplicate or mixed project roles".into());
                }
                role = Some(BuildDeclaration::Application(ApplicationDeclaration {
                    name: name.clone(),
                    artifact_only: false,
                }));
            }
            BuildRequest::Member(path) => {
                if role.is_some() {
                    return Err("Build returned mixed project roles".into());
                }
                if members.contains(path) {
                    return Err(format!(
                        "Build repeated workspace member `{}`",
                        path.as_str()
                    ));
                }
                members.push(path.clone());
            }
            BuildRequest::ArtifactOnly => {
                if artifact_only {
                    return Err("Build requested artifact-only output more than once".into());
                }
                artifact_only = true;
            }
            BuildRequest::Dependency { .. } => {}
        }
    }
    if !members.is_empty() {
        role = Some(BuildDeclaration::Workspace(WorkspaceDeclaration {
            members,
        }));
    }
    let mut role = role.ok_or("Build returned no project role")?;
    if artifact_only {
        let BuildDeclaration::Application(application) = &mut role else {
            return Err("artifact-only output requires an application role".into());
        };
        application.artifact_only = true;
    }
    Ok(role)
}
