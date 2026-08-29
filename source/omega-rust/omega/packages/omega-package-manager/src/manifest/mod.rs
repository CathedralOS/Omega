//! Read package roles and dependency rows from `build.omg`, or plan a
//! conservative dependency edit.

pub(crate) mod dependencies;
pub(crate) mod roles;

pub use dependencies::{
    BuildDependencyEditError, BuildDependencyEditPlan, BuildDependencyManualPatch,
    BuildDependencyManualReason, BuildDependencyProjection, BuildFileReplacement,
    DependencyProjectionError, DependencySourceRequest, canonical_dependency_statement,
    extract_build_dependency_projection, extract_dependency_projection, plan_dependency_addition,
    plan_dependency_replacement,
};
pub use roles::{
    ApplicationDeclaration, BuildDeclaration, BuildDeclarationError, BuildDeclarationKind,
    PackageDeclaration, PackageDeclarationError, WorkspaceDeclaration, extract_build_declaration,
    extract_package_declaration,
};
