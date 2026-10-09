//! Package identity, project roles, and dependencies read from `build.omg`.
//!
//! [`identity`] owns package names, aliases, and source-qualified identity.
//! [`roles`] owns the selected project declaration. [`dependencies`] reads exact
//! dependency rows and plans conservative edits.

pub(crate) mod dependencies;
mod identity;
pub(crate) mod roles;

pub(crate) use identity::{AliasName, PackageKey, PackageName};

pub use dependencies::edit::{
};
pub(crate) use dependencies::edit::{
    plan_dependency_addition_from_source, plan_dependency_replacement_from_source};
pub use dependencies::{ DependencyPurpose, PackageSelection,
};
pub(crate) use dependencies::{
    BuildDependencyEditError, BuildDependencyEditPlan, BuildDependencyManualPatch,
    BuildDependencyManualReason, BuildDependencyProjection, BuildFileReplacement,
    DependencyAliasError, DependencyProjectionError, DependencyProjections,
    DependencySourceRequest, ProjectedDependencies,
    canonical_dependency_statement, extract_build_dependency_projection,
    extract_dependency_projection, plan_dependency_addition, plan_dependency_replacement};
pub use roles::{
    ApplicationDeclaration, BuildDeclaration, BuildDeclarationError, BuildDeclarationKind,
    PackageDeclaration, WorkspaceDeclaration, WorkspaceMemberPath, extract_build_declaration,
};
pub(crate) use roles::{
    extract_package_declaration};

/// Flat roster position for one build-dependency purpose: product
/// dependencies occupy slot 0 and build dependencies slot 1 in the
/// per-occurrence parallel arrays the package workflows share.
pub(crate) fn purpose_slot(purpose: DependencyPurpose) -> usize {
    match purpose {
        DependencyPurpose::Product => 0,
        DependencyPurpose::Build => 1,
    }
}
