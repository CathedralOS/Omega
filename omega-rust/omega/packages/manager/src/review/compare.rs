//! Exact capability comparison between an accepted baseline and a candidate.
//!
//! [`model`] owns exact conflict values, [`limits`] bounds hostile inputs, and
//! [`error`] names fail-closed outcomes. [`capabilities`] derives row changes,
//! [`resources`] accounts hostile inputs, [`risk`] supports triage, and
//! [`root_role`] checks project-role compatibility. [`commitments`] binds those
//! changes and the candidate closure; [`format`] renders the fixed review form.

mod capabilities;
mod commitments;
mod error;
mod format;
mod limits;
mod locked_policy;
mod model;
mod render_error;
mod resources;
mod risk;
mod root_role;

pub(crate) use capabilities::compare_review_only_capability_records;
pub use capabilities::{
    compare_review_only_capabilities, compare_review_only_initial_capabilities,
};
pub use locked_policy::{LockedPolicyComparisonError, compare_locked_package_policies};
pub use model::{
    ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflict,
    ReviewOnlyCapabilityConflictBaseline, ReviewOnlyCapabilityConflictChange,
    ReviewOnlyCapabilityConflictError, ReviewOnlyCapabilityConflictFingerprint,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyCapabilityConflictSet,
    ReviewOnlyPackageCapabilityConflicts, ReviewOnlyRootRoleChange,
    ReviewOnlyRootRoleComparisonError, ReviewOnlyRootRoleContract, ReviewSetRole,
};
pub use render_error::ReviewOnlyCapabilityConflictRenderError;
pub(crate) use risk::changed_review_risk;
pub(crate) use root_role::compare_review_only_root_role_graphs;

#[cfg(test)]
mod tests;
