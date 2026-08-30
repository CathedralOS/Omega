//! Candidate-bound root-policy decisions for blocking review conflicts.

mod model;
mod record;
mod resolution;
mod storage;

pub use model::{ReviewOnlyRootPolicyDecision, ReviewOnlyRootPolicyDisposition};
pub use record::{
    ReviewOnlyRootPolicyRecordError, ReviewOnlyRootPolicyRecordLimits,
    recover_review_only_root_policy_resolution,
};
pub use resolution::{
    ReviewOnlyRootPolicyResolution, ReviewOnlyRootPolicyResolutionCommitment,
    ReviewOnlyRootPolicyResolutionError, resolve_review_only_root_policy_decisions,
};
pub use storage::{
    ReviewOnlyRootPolicyDirectory, ReviewOnlyRootPolicyFileError, ReviewOnlyRootPolicyName,
    ReviewOnlyRootPolicyNameError,
};
