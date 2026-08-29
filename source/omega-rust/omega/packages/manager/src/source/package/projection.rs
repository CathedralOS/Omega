use super::ResolvePackageSourceError;
use crate::manifest::declaration::{BuildDeclaration, PackageDeclaration, PackageDeclarationError};
use crate::manifest::dependency_projection::{
    DependencyProjectionError, DependencySourceRequest, extract_build_dependency_projection,
};
use std::path::Path;

pub(super) fn project_package_build(
    snapshot_root: &Path,
    application_root_allowed: bool,
) -> Result<(PackageDeclaration, Vec<DependencySourceRequest>), ResolvePackageSourceError> {
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
        BuildDeclaration::Package(package) => Ok((package, dependencies)),
        BuildDeclaration::Application(application) if application_root_allowed => Ok((
            PackageDeclaration {
                name: application.name,
            },
            dependencies,
        )),
        other => Err(ResolvePackageSourceError::Declaration(
            PackageDeclarationError::ExpectedPackageDeclaration {
                found: other.kind(),
            },
        )),
    }
}
