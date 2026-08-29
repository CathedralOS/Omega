//! Exact capability comparison between an accepted baseline and a candidate.
//!
//! [`model`] owns the bounded conflict vocabulary, [`compare`] derives exact
//! row changes and commitments, and [`format`] renders the fixed review form.

mod compare;
mod format;
mod model;

pub use compare::compare_review_only_capabilities;
pub(crate) use compare::{changed_review_risk, compare_review_only_capability_records};
pub use model::{
    ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflict,
    ReviewOnlyCapabilityConflictChange, ReviewOnlyCapabilityConflictError,
    ReviewOnlyCapabilityConflictFingerprint, ReviewOnlyCapabilityConflictLimits,
    ReviewOnlyCapabilityConflictRenderError, ReviewOnlyCapabilityConflictSet,
    ReviewOnlyPackageCapabilityConflicts, ReviewSetRole,
};

#[cfg(test)]
mod tests;
