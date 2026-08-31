//! Exact capability comparison between an accepted baseline and a candidate.
//!
//! [`model`] owns exact conflict values, [`limits`] bounds hostile inputs,
//! [`error`] names fail-closed outcomes, and [`compare`] derives row changes.
//! [`commitments`] binds those changes and the candidate closure; [`format`]
//! renders the fixed review form.

mod commitments;
mod compare;
mod error;
mod format;
mod limits;
mod model;
mod render_error;

pub(crate) use compare::{
    changed_review_risk, compare_review_only_capability_records,
    compare_review_only_root_role_graphs,
};
pub use compare::{compare_review_only_capabilities, compare_review_only_initial_capabilities};
pub use model::{
    ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflict,
    ReviewOnlyCapabilityConflictBaseline, ReviewOnlyCapabilityConflictChange,
    ReviewOnlyCapabilityConflictError, ReviewOnlyCapabilityConflictFingerprint,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyCapabilityConflictSet,
    ReviewOnlyPackageCapabilityConflicts, ReviewOnlyRootRoleChange,
    ReviewOnlyRootRoleComparisonError, ReviewOnlyRootRoleContract, ReviewSetRole,
};
pub use render_error::ReviewOnlyCapabilityConflictRenderError;

#[cfg(test)]
mod tests;
