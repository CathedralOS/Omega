//! Dependency rows within `build.omg`.
//!
//! [`read`] projects exact authored rows without evaluating build logic.
//! [`edit`] produces digest-bound conservative changes to those rows.

pub(crate) mod edit;
pub(crate) mod read;

pub(crate) use edit::{
    BuildDependencyEditError, BuildDependencyEditPlan, BuildDependencyManualPatch,
    BuildDependencyManualReason, BuildFileReplacement, canonical_dependency_statement,
};
pub(crate) use read::{
    BuildDependencyProjection, DependencyAliasError, DependencyProjectionError,
    DependencyProjections, DependencyPurpose, DependencySourceRequest, PackageSelection,
    ProjectedDependencies, extract_build_dependency_projection,
};
