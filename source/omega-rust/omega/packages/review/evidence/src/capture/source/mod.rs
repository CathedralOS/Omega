//! Authored source custody, compiler-private row pairings, and finalization.

pub(super) mod contracts;
pub(super) mod finalization;
pub(super) mod invocations;
pub(super) mod locations;
pub(super) mod parameters;
mod rows;
pub(super) mod service_reach;
pub(super) mod suspension;

pub(crate) use rows::{
    ProjectedDangerousAuthorityRow, ProjectedDangerousAuthoritySlackRow,
    ProjectedNestedSourceLocation, ProjectedReviewRow, ProjectedSemanticDependencyRow,
};
