//! Dependency rows within `build.omg`.
//!
//! [`read`] projects exact authored rows without evaluating build logic.
//! [`edit`] produces digest-bound conservative changes to those rows.

pub(crate) mod edit;
pub(crate) mod read;

pub use edit::{
    BuildDependencyEditError, BuildDependencyEditPlan, BuildDependencyManualPatch,
    BuildDependencyManualReason, BuildFileReplacement, canonical_dependency_statement,
    plan_dependency_addition, plan_dependency_replacement,
};
pub use read::{
    BuildDependencyProjection, DependencyProjectionError, DependencySourceRequest,
    extract_build_dependency_projection, extract_dependency_projection,
};
