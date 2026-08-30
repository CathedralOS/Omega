//! Package, application, and workspace roles declared by `build.omg`.

use crate::manifest::PackageName;
use omega_build_declarations as shared;
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDeclaration {
    pub members: Vec<WorkspaceMemberPath>,
}

/// Project the package-authored human name from the immutable package root.
///
/// This preserves the package-manager identity-bearing API while the syntax
/// authority lives in `omega-build-declarations`.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_from_source(source: &str) -> Result<BuildDeclaration, BuildDeclarationError> {
        shared::project_build_declaration_from_source(source).map(convert_shared_declaration)
    }

    #[test]
    fn wrapper_preserves_package_manager_identity_types() {
        let declaration = extract_from_source(
            r#"machine build(builder: &mut Build) { builder.package("arithmetic-kernels"); }"#,
        )
        .expect("project package");

        assert_eq!(
            declaration,
            BuildDeclaration::Package(PackageDeclaration {
                name: PackageName::parse("arithmetic-kernels").unwrap(),
            })
        );
    }

    #[test]
    fn wrapper_preserves_exact_single_builder_parameter_rule() {
        assert_eq!(
            extract_from_source(
                r#"machine build(builder: &mut Build, filesystem: &mut Filesystem) {}"#,
            ),
            Err(BuildDeclarationError::InvalidBuildParameter)
        );
        assert_eq!(
            extract_from_source(
                r#"machine build(filesystem: &mut Filesystem, builder: &mut Build) {}"#,
            ),
            Err(BuildDeclarationError::InvalidBuildParameter)
        );
        assert!(
            extract_from_source(
                r#"machine build(builder: &mut Build) {
                    builder.application("service-backed-app");
                }"#,
            )
            .is_ok()
        );
    }

    #[test]
    fn package_only_wrapper_retains_legacy_missing_and_wrong_kind_errors() {
        assert_eq!(
            extract_from_source("machine build(builder: &mut Build) {}"),
            Err(BuildDeclarationError::MissingBuildDeclaration)
        );
        let application = extract_from_source(
            r#"machine build(builder: &mut Build) { builder.application("app"); }"#,
        )
        .unwrap();
        assert_eq!(application.kind(), BuildDeclarationKind::Application);
    }
}
