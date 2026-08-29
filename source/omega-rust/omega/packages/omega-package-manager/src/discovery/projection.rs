use super::ResolvePackageSourceError;
use crate::declarations::dependencies::read::{
    DependencyProjectionError, ProjectedDependencies, extract_build_dependency_projection,
};
use crate::declarations::project::{
    BuildDeclaration, BuildDeclarationKind, PackageDeclarationError,
};
use crate::identity::PackageName;
use std::path::Path;

/// The identity-bearing declaration projected from one selected source root.
///
/// The declared role remains separate from `PackageKey`: roots may be packages
/// or applications, while dependency adapters accept packages only.
pub(super) struct ProjectedPackageBuild {
    pub(super) name: PackageName,
    pub(super) role: BuildDeclarationKind,
    pub(super) dependencies: ProjectedDependencies,
}

pub(super) fn project_package_build(
    snapshot_root: &Path,
    application_root_allowed: bool,
) -> Result<ProjectedPackageBuild, ResolvePackageSourceError> {
    let projection = match extract_build_dependency_projection(snapshot_root) {
        Ok(projection) => projection,
        Err(DependencyProjectionError::MissingBuildFile { path }) => {
            return Err(ResolvePackageSourceError::Declaration(
                PackageDeclarationError::MissingBuildFile { path },
            ));
        }
        Err(DependencyProjectionError::ReadBuildFile { path, message }) => {
            return Err(ResolvePackageSourceError::Declaration(
                PackageDeclarationError::ReadBuildFile { path, message },
            ));
        }
        Err(DependencyProjectionError::InvalidBuildFileEncoding { path }) => {
            return Err(ResolvePackageSourceError::Declaration(
                PackageDeclarationError::InvalidBuildFileEncoding { path },
            ));
        }
        Err(DependencyProjectionError::Lex { message }) => {
            return Err(ResolvePackageSourceError::Declaration(
                PackageDeclarationError::Lex { message },
            ));
        }
        Err(DependencyProjectionError::Parse { message }) => {
            return Err(ResolvePackageSourceError::Declaration(
                PackageDeclarationError::Parse { message },
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
        BuildDeclaration::Application(application) if application_root_allowed => {
            Ok(ProjectedPackageBuild {
                name: application.name,
                role: BuildDeclarationKind::Application,
                dependencies,
            })
        }
        other => Err(ResolvePackageSourceError::Declaration(
            PackageDeclarationError::ExpectedPackageDeclaration {
                found: other.kind(),
            },
        )),
    }
}
