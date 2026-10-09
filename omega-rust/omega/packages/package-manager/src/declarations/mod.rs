//! Package identity, project roles, and dependencies read from `build.omg`.
//!
//! [`identity`] owns package names, aliases, and source-qualified identity.
//! [`roles`] owns the selected project declaration. [`dependencies`] reads exact
//! dependency rows and plans conservative edits.

pub(crate) mod dependencies;
mod identity;
pub(crate) mod roles;

pub(crate) use identity::{AliasName, PackageKey, PackageName};
pub(crate) use dependencies::edit::{
    plan_dependency_addition_from_source, plan_dependency_replacement_from_source,
};
pub(crate) use dependencies::{ BuildDependencyEditPlan, BuildFileReplacement, DependencyProjectionError, DependencyPurpose,
    DependencySourceRequest, PackageSelection,
};
pub(crate) use roles::{ BuildDeclarationError, BuildDeclarationKind,
};

/// Flat roster position for one build-dependency purpose: product
/// dependencies occupy slot 0 and build dependencies slot 1 in the
/// per-occurrence parallel arrays the package workflows share.
pub(crate) fn purpose_slot(purpose: DependencyPurpose) -> usize {
    match purpose {
        DependencyPurpose::Product => 0,
        DependencyPurpose::Build => 1,
    }
}
