//! Package identity, project roles, and dependencies read from `build.omg`.
//!
//! [`identity`] owns package names, aliases, and source-qualified identity.
//! [`dependencies`] reads exact dependency rows and plans conservative edits.
//! The project-role vocabulary itself is owned by `build_declarations` and
//! re-exported here rather than restated.

pub(crate) mod dependencies;
mod identity;

pub(crate) use identity::{AliasName, PackageKey, PackageName};

pub use build_declarations::{BuildDeclarationError, BuildDeclarationKind, WorkspaceMemberPath};
pub(crate) use dependencies::edit::{
    plan_dependency_addition_from_source, plan_dependency_replacement_from_source,
};
pub(crate) use dependencies::{
    BuildDependencyEditPlan, BuildFileReplacement, DependencyProjectionError,
    DependencySourceRequest,
};
pub use dependencies::{DependencyPurpose, PackageSelection};

/// Flat roster position for one build-dependency purpose: product
/// dependencies occupy slot 0 and build dependencies slot 1 in the
/// per-occurrence parallel arrays the package workflows share.
pub(crate) fn purpose_slot(purpose: DependencyPurpose) -> usize {
    match purpose {
        DependencyPurpose::Product => 0,
        DependencyPurpose::Build => 1,
    }
}
