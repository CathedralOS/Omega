//! A Build produces either a project declaration or the next interpreted Build entry.

use build_declarations::{
    ApplicationDeclaration, BuildDeclaration, BuildEntryPath, BuildRequest, PackageDeclaration,
    WorkspaceDeclaration,
};

use crate::host::requests::IssuedBuildRequest;

pub(super) enum Destination {
    Project(BuildDeclaration),
    Successor(BuildEntryPath),
}

impl Destination {
    pub(super) fn project(&self) -> Option<&BuildDeclaration> {
        match self {
            Self::Project(declaration) => Some(declaration),
            Self::Successor(_) => None,
        }
    }
}

pub(super) fn destination(requests: &[IssuedBuildRequest]) -> Result<Destination, String> {
    use BuildDeclaration::{Application, Package, Workspace};
    use Destination::{Project, Successor};

    let mut destination = None;
    let mut artifact_only = false;
    for issued in requests {
        let next = match issued.request() {
            BuildRequest::Package(name) => {
                Project(Package(PackageDeclaration { name: name.clone() }))
            }
            BuildRequest::Application(name) => Project(Application(ApplicationDeclaration {
                name: name.clone(),
                artifact_only: false,
            })),
            BuildRequest::BuildEntry(path) => Successor(path.clone()),
            BuildRequest::Member(path) => {
                if let Some(Project(Workspace(workspace))) = &mut destination {
                    if workspace.members.contains(path) {
                        return Err(format!(
                            "Build repeated workspace member `{}`",
                            path.as_str()
                        ));
                    }
                    workspace.members.push(path.clone());
                    continue;
                }
                Project(Workspace(WorkspaceDeclaration {
                    members: vec![path.clone()],
                }))
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
        if let Some(previous) = destination {
            return Err(match (previous, next) {
                (Project(Package(previous)), Project(Package(next))) => format!(
                    "Build declared the package project role more than once (`{}` and `{}`)",
                    previous.name.as_str(),
                    next.name.as_str()
                ),
                (Project(Application(previous)), Project(Application(next))) => format!(
                    "Build declared the application project role more than once (`{}` and `{}`)",
                    previous.name.as_str(),
                    next.name.as_str()
                ),
                (Successor(previous), Successor(next)) => format!(
                    "Build selected a successor more than once (`{}` and `{}`)",
                    previous.as_str(),
                    next.as_str()
                ),
                _ => "Build returned mixed project roles".into(),
            });
        }
        destination = Some(next);
    }
    let mut destination = destination.ok_or("Build returned no project role")?;
    if artifact_only {
        let Project(Application(application)) = &mut destination else {
            return Err("artifact-only output requires an application role".into());
        };
        application.artifact_only = true;
    }
    Ok(destination)
}
