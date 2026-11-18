/*
Normalize the authored project role (`BuildDeclaration`) from the activation's
issued request ledger at normal return (`declaration`).

An authored `build.omg` must declare exactly one role (`Build::package`,
`Build::application`, or one or more `Build::member` workspace entries), and
`Build::artifact_only` is permitted at most once and only on an `Application`
role.
*/

use build_declarations::{
    ApplicationDeclaration, BuildDeclaration, BuildRequest, PackageDeclaration,
    WorkspaceDeclaration,
};

use crate::host::requests::IssuedBuildRequest;

pub(super) fn declaration(requests: &[IssuedBuildRequest]) -> Result<BuildDeclaration, String> {
    let mut role = None;
    let mut members = Vec::new();
    let mut artifact_only = false;
    for issued in requests {
        let next_role = match issued.request() {
            BuildRequest::Package(name) => {
                BuildDeclaration::Package(PackageDeclaration { name: name.clone() })
            }
            BuildRequest::Application(name) => {
                BuildDeclaration::Application(ApplicationDeclaration {
                    name: name.clone(),
                    artifact_only: false,
                })
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
                continue;
            }
            BuildRequest::ArtifactOnly => {
                if artifact_only {
                    return Err("Build requested artifact-only output more than once".into());
                }
                artifact_only = true;
                continue;
            }
            BuildRequest::Dependency { .. } => continue,
        };
        match (&role, &next_role) {
            (Some(BuildDeclaration::Package(previous)), BuildDeclaration::Package(next)) => {
                return Err(format!(
                    "Build declared the package role more than once (`{}` and `{}`)",
                    previous.name.as_str(),
                    next.name.as_str()
                ));
            }
            (
                Some(BuildDeclaration::Application(previous)),
                BuildDeclaration::Application(next),
            ) => {
                return Err(format!(
                    "Build declared the application role more than once (`{}` and `{}`)",
                    previous.name.as_str(),
                    next.name.as_str()
                ));
            }
            _ if role.is_some() || !members.is_empty() => {
                return Err("Build returned mixed project roles".into());
            }
            _ => role = Some(next_role),
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
