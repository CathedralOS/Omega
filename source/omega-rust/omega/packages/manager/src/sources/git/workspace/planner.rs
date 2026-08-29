use super::{
    BuildDeclarationEvidence, GitWorkspaceDiscovery, GitWorkspaceMemberBuild,
    GitWorkspaceMemberPlan, GitWorkspaceSelectionError, GitWorkspaceSelectionLimit,
    GitWorkspaceSelectionPlan,
};
use crate::project::dependencies::read::validate_static_dependency_source;
use crate::project::roles::project_build_declaration_source;
use omega_build_declarations::{
    BuildDeclaration, BuildDeclarationKind, ProjectName, WorkspaceMemberPath,
};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_BUILD_DECLARATION_BYTES: usize = 1024 * 1024;
pub const MAX_TOTAL_BUILD_DECLARATION_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_WORKSPACE_MEMBERS: usize = 4096;

pub fn discover_git_workspace(
    root_build_bytes: &[u8],
) -> Result<GitWorkspaceDiscovery, GitWorkspaceSelectionError> {
    check_declaration_size(root_build_bytes)?;
    let root_source = decode_declaration("build.omg", root_build_bytes)?;
    let root_declaration = project_build_declaration_source(root_source).map_err(|error| {
        GitWorkspaceSelectionError::MalformedDeclaration {
            repository_path: "build.omg".to_owned(),
            error,
        }
    })?;
    let BuildDeclaration::Workspace(workspace) = root_declaration else {
        return Err(GitWorkspaceSelectionError::WrongRole {
            repository_path: "build.omg".to_owned(),
            expected: BuildDeclarationKind::Workspace,
            found: root_declaration.kind(),
        });
    };
    if workspace.members.len() > MAX_WORKSPACE_MEMBERS {
        return Err(GitWorkspaceSelectionError::ResourceLimit {
            limit: GitWorkspaceSelectionLimit::WorkspaceMembers,
            maximum: MAX_WORKSPACE_MEMBERS,
            observed: workspace.members.len(),
        });
    }
    Ok(GitWorkspaceDiscovery::new(
        BuildDeclarationEvidence::from_bytes("build.omg".to_owned(), root_build_bytes),
        workspace.members,
    ))
}

pub fn plan_git_workspace_selection(
    selected_package: &ProjectName,
    root_build_bytes: &[u8],
    member_builds: &[GitWorkspaceMemberBuild<'_>],
) -> Result<GitWorkspaceSelectionPlan, GitWorkspaceSelectionError> {
    let discovery = discover_git_workspace(root_build_bytes)?;

    let declared_paths = discovery
        .member_paths()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut supplied = BTreeMap::new();
    let mut total_bytes = account_declaration_bytes(0, root_build_bytes)?;
    for member_build in member_builds {
        total_bytes = account_declaration_bytes(total_bytes, member_build.build_bytes())?;
        if supplied
            .insert(
                member_build.member_path().clone(),
                member_build.build_bytes(),
            )
            .is_some()
        {
            return Err(GitWorkspaceSelectionError::DuplicateMemberBuild {
                member_path: member_build.member_path().clone(),
            });
        }
    }

    if let Some(member_path) = discovery
        .member_paths()
        .iter()
        .find(|member_path| !supplied.contains_key(*member_path))
    {
        return Err(GitWorkspaceSelectionError::MissingMemberBuild {
            member_path: member_path.clone(),
        });
    }
    if let Some(member_path) = supplied
        .keys()
        .find(|member_path| !declared_paths.contains(*member_path))
    {
        return Err(GitWorkspaceSelectionError::ExtraMemberBuild {
            member_path: member_path.clone(),
        });
    }

    let mut members = Vec::with_capacity(discovery.member_paths().len());
    let mut matches = Vec::new();
    for member_path in discovery.member_paths().iter().cloned() {
        let bytes = supplied
            .get(&member_path)
            .expect("declared member set was proven complete");
        let repository_path = member_build_path(&member_path);
        let source = decode_declaration(&repository_path, bytes)?;
        let declaration = project_build_declaration_source(source).map_err(|error| {
            GitWorkspaceSelectionError::MalformedDeclaration {
                repository_path: repository_path.clone(),
                error,
            }
        })?;
        let (project_name, role) = match declaration {
            BuildDeclaration::Package(package) => (package.name, BuildDeclarationKind::Package),
            BuildDeclaration::Application(application) => {
                (application.name, BuildDeclarationKind::Application)
            }
            BuildDeclaration::Workspace(_) => {
                return Err(GitWorkspaceSelectionError::WrongRole {
                    repository_path,
                    expected: BuildDeclarationKind::Package,
                    found: BuildDeclarationKind::Workspace,
                });
            }
        };
        validate_static_dependency_source(source).map_err(|error| {
            GitWorkspaceSelectionError::StaticDependencyProjection {
                member_path: member_path.clone(),
                error,
            }
        })?;
        if &project_name == selected_package {
            matches.push(member_path.clone());
        }
        members.push(GitWorkspaceMemberPlan::new(
            member_path,
            project_name,
            role,
            BuildDeclarationEvidence::from_bytes(repository_path, bytes),
        ));
    }

    let selected_member_path = match matches.as_slice() {
        [] => {
            return Err(GitWorkspaceSelectionError::PackageMissing {
                package_name: selected_package.clone(),
            });
        }
        [member_path] => member_path.clone(),
        _ => {
            return Err(GitWorkspaceSelectionError::PackageDuplicate {
                package_name: selected_package.clone(),
                member_paths: matches,
            });
        }
    };

    Ok(GitWorkspaceSelectionPlan::new(
        selected_member_path,
        discovery.workspace_declaration().clone(),
        members,
    ))
}

fn check_declaration_size(bytes: &[u8]) -> Result<(), GitWorkspaceSelectionError> {
    if bytes.len() > MAX_BUILD_DECLARATION_BYTES {
        return Err(GitWorkspaceSelectionError::ResourceLimit {
            limit: GitWorkspaceSelectionLimit::DeclarationBytes,
            maximum: MAX_BUILD_DECLARATION_BYTES,
            observed: bytes.len(),
        });
    }
    Ok(())
}

pub(crate) fn account_declaration_bytes(
    current: usize,
    bytes: &[u8],
) -> Result<usize, GitWorkspaceSelectionError> {
    check_declaration_size(bytes)?;
    let observed = current.checked_add(bytes.len()).unwrap_or(usize::MAX);
    if observed > MAX_TOTAL_BUILD_DECLARATION_BYTES {
        return Err(GitWorkspaceSelectionError::ResourceLimit {
            limit: GitWorkspaceSelectionLimit::TotalDeclarationBytes,
            maximum: MAX_TOTAL_BUILD_DECLARATION_BYTES,
            observed,
        });
    }
    Ok(observed)
}

fn decode_declaration<'a>(
    repository_path: &str,
    bytes: &'a [u8],
) -> Result<&'a str, GitWorkspaceSelectionError> {
    std::str::from_utf8(bytes).map_err(|_| GitWorkspaceSelectionError::NonUtf8Declaration {
        repository_path: repository_path.to_owned(),
    })
}

fn member_build_path(member_path: &WorkspaceMemberPath) -> String {
    format!("{}/build.omg", member_path.as_str())
}
