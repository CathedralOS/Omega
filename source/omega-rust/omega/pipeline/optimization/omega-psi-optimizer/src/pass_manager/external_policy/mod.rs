//! External-policy recording and replay entrance.
//!
//! [`expected_context`] binds the complete compiler-authored request context;
//! [`candidate_features`] projects only independently validated candidates;
//! replay admits an action only after exact feature comparison, while
//! recording reconstructs the same surface independently from manifests.

mod candidate_features;
mod context;
mod recording;
mod replay;

use omega_optimization_core::OptimizationRuleContract;
use omega_optimization_policy::{ExternalCandidateFeatures, ExternalDecisionSchemaError};
use omega_optimization_unit::PsiRewriteCandidate;

pub(super) use context::expected_context;
pub(super) use recording::external_points_from_manifest_decisions;
pub use recording::validate_external_decision_recording;
pub(super) use replay::ExternalDecisionReplayCursor;

/// Join the validated candidate to its scheduled rule contract at the sole
/// policy-input entrance. Leaves own projection and replay; this boundary owns
/// which evidence may leave the pass manager.
pub(super) fn validated_candidate_features(
    candidate: &PsiRewriteCandidate,
    contract: OptimizationRuleContract,
) -> Result<ExternalCandidateFeatures, ExternalDecisionSchemaError> {
    candidate_features::derive(candidate, contract)
}
