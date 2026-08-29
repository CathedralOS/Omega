//! Compiler-private row/source pairings and canonical source finalization.

pub(super) mod finalization;
pub(super) mod locations;
mod model;

pub(crate) use model::{
    ProjectedDangerousAuthorityRow, ProjectedDangerousAuthoritySlackRow,
    ProjectedNestedSourceLocation, ProjectedReviewRow, ProjectedSemanticDependencyRow,
};
