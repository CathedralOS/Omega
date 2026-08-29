//! Exact capability comparison between an accepted baseline and a candidate.
//!
//! [`model`] owns the bounded conflict vocabulary, [`compare`] derives exact
//! row changes, [`commitments`] binds those changes and the candidate closure,
//! and [`format`] renders the fixed review form.

mod commitments;
mod compare;
mod format;
mod model;
mod render_error;

pub use compare::compare_review_only_capabilities;
pub(crate) use compare::{
    changed_review_risk, compare_review_only_capability_records,
    compare_review_only_root_role_graphs,
};
pub use model::{
    ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflict,
    ReviewOnlyCapabilityConflictChange, ReviewOnlyCapabilityConflictError,
    ReviewOnlyCapabilityConflictFingerprint, ReviewOnlyCapabilityConflictLimits,
    ReviewOnlyCapabilityConflictSet, ReviewOnlyPackageCapabilityConflicts,
    ReviewOnlyRootRoleChange, ReviewOnlyRootRoleComparisonError, ReviewOnlyRootRoleContract,
    ReviewSetRole,
};
pub use render_error::ReviewOnlyCapabilityConflictRenderError;

#[cfg(test)]
mod tests;
