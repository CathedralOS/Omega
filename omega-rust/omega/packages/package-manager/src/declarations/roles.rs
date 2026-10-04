//! Package, application, and workspace roles declared by `build.omg`.

use crate::declarations::PackageName;
use build_declarations as shared;
use std::path::Path;

pub use shared::BuildDeclarationError;
pub use shared::BuildDeclarationKind;
pub use shared::WorkspaceMemberPath;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildDeclaration {
    Package(PackageDeclaration),
    Application(ApplicationDeclaration),
    Workspace(WorkspaceDeclaration),
}

impl BuildDeclaration {
    pub const fn kind(&self) -> BuildDeclarationKind {
        match self {
            Self::Package(_) => BuildDeclarationKind::Package,
            Self::Application(_) => BuildDeclarationKind::Application,
            Self::Workspace(_) => BuildDeclarationKind::Workspace,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDeclaration {
    pub name: PackageName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationDeclaration {
    pub name: PackageName,
    /// The application declared `Build::artifact_only(builder)` in its root build
    /// entry: the build publishes only the staged artifact set and admits no
    /// executable route.
    pub artifact_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDeclaration {
    pub members: Vec<WorkspaceMemberPath>,
}

/// Project the package-authored human name from the immutable package root.
///
/// This preserves the package-manager identity-bearing API while the syntax
/// authority lives in `build-declarations`.
pub fn extract_package_declaration(
    package_root: impl AsRef<Path>,
) -> Result<PackageDeclaration, BuildDeclarationError> {
    match extract_build_declaration(package_root) {
        Ok(BuildDeclaration::Package(declaration)) => Ok(declaration),
        Ok(other) => Err(BuildDeclarationError::ExpectedPackageDeclaration {
            found: other.kind(),
        }),
        Err(BuildDeclarationError::MissingBuildDeclaration) => {
            Err(BuildDeclarationError::MissingPackageDeclaration)
        }
        Err(error) => Err(error),
    }
}

/// Project the explicit package, application, or workspace kind from one
/// immutable root `build.omg` without executing build code.
pub fn extract_build_declaration(
    root: impl AsRef<Path>,
) -> Result<BuildDeclaration, BuildDeclarationError> {
    shared::extract_build_declaration(root).map(convert_shared_declaration)
}

pub(crate) fn convert_shared_declaration(
    declaration: shared::BuildDeclaration,
) -> BuildDeclaration {
    match declaration {
        shared::BuildDeclaration::Package(declaration) => {
            BuildDeclaration::Package(PackageDeclaration {
                name: convert_project_name(declaration.name),
            })
        }
        shared::BuildDeclaration::Application(declaration) => {
            BuildDeclaration::Application(ApplicationDeclaration {
                name: convert_project_name(declaration.name),
                artifact_only: declaration.artifact_only,
            })
        }
        shared::BuildDeclaration::Workspace(declaration) => {
            BuildDeclaration::Workspace(WorkspaceDeclaration {
                members: declaration.members,
            })
        }
    }
}

/// Project one already-decoded `build.omg` without consulting the filesystem.
///
/// Manager-owned planners that must remain independent of package-source
/// identity types use the declaration-domain result directly.
pub(crate) fn project_build_declaration_source(
    source: &str,
) -> Result<shared::BuildDeclaration, BuildDeclarationError> {
    shared::project_build_declaration_from_source(source)
}

fn convert_project_name(name: shared::ProjectName) -> PackageName {
    name.into()
}
