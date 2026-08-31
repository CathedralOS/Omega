//! Package identity, project roles, and dependencies read from `build.omg`.
//!
//! [`identity`] owns package names, aliases, and source-qualified identity.
//! [`roles`] owns the selected project declaration. [`dependencies`] reads exact
//! dependency rows and plans conservative edits.

pub(crate) mod dependencies;
mod identity;
pub(crate) mod roles;

pub use identity::{AliasName, PackageKey, PackageName};

pub use dependencies::{
    BuildDependencyEditError, BuildDependencyEditPlan, BuildDependencyManualPatch,
    BuildDependencyManualReason, BuildDependencyProjection, BuildFileReplacement,
    DependencyAliasError, DependencyProjectionError, DependencySourceRequest, PackageSelection,
    ProjectedDependencies, canonical_dependency_statement, extract_build_dependency_projection,
    extract_dependency_projection, plan_dependency_addition, plan_dependency_replacement,
};
pub use roles::{
    ApplicationDeclaration, BuildDeclaration, BuildDeclarationError, BuildDeclarationKind,
    PackageDeclaration, WorkspaceDeclaration, WorkspaceMemberPath, extract_build_declaration,
    extract_package_declaration,
};

#[cfg(test)]
mod identity_tests;
