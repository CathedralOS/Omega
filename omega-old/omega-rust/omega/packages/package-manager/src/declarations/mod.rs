/*
Package identity, project roles, and dependencies read from `build.omg`.

`identity` owns package names, aliases, and source-qualified identity.
`dependencies` reads exact dependency rows and plans conservative edits. The
project-role vocabulary itself is owned by `build_declarations` and re-exported
here rather than restated. `purpose_slot` maps `DependencyPurpose::Product` to
slot 0 and `DependencyPurpose::Build` to slot 1 in the per-occurrence parallel
arrays shared across package workflows.
*/

pub(crate) mod dependencies;
mod identity;

pub(crate) use identity::AliasName;
pub use identity::{PackageKey, PackageName};

pub use build_declarations::{BuildDeclarationError, BuildDeclarationKind, WorkspaceMemberPath};
pub use dependencies::DependencyPurpose;
pub(crate) use dependencies::PackageSelection;
pub(crate) use dependencies::edit::{
    plan_dependency_addition_from_source, plan_dependency_replacement_from_source,
};
pub(crate) use dependencies::{
    BuildDependencyEditPlan, BuildFileReplacement, DependencySourceRequest,
};

pub(crate) fn purpose_slot(purpose: DependencyPurpose) -> usize {
    match purpose {
        DependencyPurpose::Product => 0,
        DependencyPurpose::Build => 1,
    }
}
