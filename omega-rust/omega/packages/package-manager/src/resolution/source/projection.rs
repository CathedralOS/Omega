use super::{ResolvePackageSourceError, SourceRootRole};
use crate::declarations::PackageName;
use crate::declarations::dependencies::read::{
    DependencyProjectionError, DependencyProjections, extract_build_dependency_projection,
};
use crate::declarations::roles::{BuildDeclaration, BuildDeclarationError, BuildDeclarationKind};
use std::path::Path;

/// The identity-bearing declaration projected from one selected source root.
///
/// The declared role remains separate from `PackageKey`: roots may be packages
/// or applications, while dependency adapters accept packages only.
pub(super) struct ProjectedPackageBuild {
    pub(super) name: PackageName,
    pub(super) role: BuildDeclarationKind,
    pub(super) dependencies: DependencyProjections,
}

pub(super) fn project_package_build(
    snapshot_root: &Path,
    role: SourceRootRole,
) -> Result<ProjectedPackageBuild, ResolvePackageSourceError> {
    project_build_projection(extract_build_dependency_projection(snapshot_root), role)
}

pub(super) fn project_captured_package_build(
    root: &Path,
    tree: &source::CapturedSourceTree,
    role: SourceRootRole,
) -> Result<ProjectedPackageBuild, ResolvePackageSourceError> {
    let path = root.join("build.omg");
    let text = tree
        .read_to_string(Path::new("build.omg"))
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ResolvePackageSourceError::Declaration(BuildDeclarationError::MissingBuildFile {
                    path: path.clone(),
                })
            } else if error.kind() == std::io::ErrorKind::InvalidData {
                ResolvePackageSourceError::Declaration(
                    BuildDeclarationError::InvalidBuildFileEncoding { path: path.clone() },
                )
            } else {
                ResolvePackageSourceError::Declaration(BuildDeclarationError::ReadBuildFile {
                    path: path.clone(),
                    message: error.to_string(),
                })
            }
        })?;
    project_build_projection(
        crate::declarations::dependencies::read::extract_build_projection_from_source(&text),
        role,
    )
}

fn project_build_projection(
    result: Result<
        crate::declarations::dependencies::read::BuildDependencyProjection,
        DependencyProjectionError,
    >,
    role: SourceRootRole,
) -> Result<ProjectedPackageBuild, ResolvePackageSourceError> {
    let projection = match result {
        Ok(projection) => projection,
        Err(DependencyProjectionError::MissingBuildFile { path }) => {
            return Err(ResolvePackageSourceError::Declaration(
                BuildDeclarationError::MissingBuildFile { path },
            ));
        }
        Err(DependencyProjectionError::ReadBuildFile { path, message }) => {
            return Err(ResolvePackageSourceError::Declaration(
                BuildDeclarationError::ReadBuildFile { path, message },
            ));
        }
        Err(DependencyProjectionError::InvalidBuildFileEncoding { path }) => {
            return Err(ResolvePackageSourceError::Declaration(
                BuildDeclarationError::InvalidBuildFileEncoding { path },
            ));
        }
        Err(DependencyProjectionError::Lex { message }) => {
            return Err(ResolvePackageSourceError::Declaration(
                BuildDeclarationError::Lex { message },
            ));
        }
        Err(DependencyProjectionError::Parse { message }) => {
            return Err(ResolvePackageSourceError::Declaration(
                BuildDeclarationError::Parse { message },
            ));
        }
        Err(DependencyProjectionError::BuildDeclaration(error)) => {
            return Err(ResolvePackageSourceError::Declaration(*error));
        }
        Err(error) => return Err(ResolvePackageSourceError::DependencyProjection(error)),
    };
    let (declaration, dependencies) = projection.into_parts();
    match declaration {
        BuildDeclaration::Package(package) => Ok(ProjectedPackageBuild {
            name: package.name,
            role: BuildDeclarationKind::Package,
            dependencies,
        }),
        BuildDeclaration::Application(application) if role.allows_application_root() => {
            Ok(ProjectedPackageBuild {
                name: application.name,
                role: BuildDeclarationKind::Application,
                dependencies,
            })
        }
        other => Err(ResolvePackageSourceError::Declaration(
            BuildDeclarationError::ExpectedPackageDeclaration {
                found: other.kind(),
            },
        )),
    }
}
