use omega_optimization_core::OptimizationRuleContract;
use omega_optimization_core::{
    ExternalCandidateFeatures, ExternalDecisionSchemaError, ValidatedCandidateSummary,
};
use omega_optimization_unit::PsiRewriteCandidate;

/// Project the exact policy-visible row only after the ordinary candidate
/// validator has admitted the candidate. Analysis features come from the
/// scheduled contract; facts come from the immutable candidate declaration.
pub(super) fn derive(
    candidate: &PsiRewriteCandidate,
    contract: OptimizationRuleContract,
) -> Result<ExternalCandidateFeatures, ExternalDecisionSchemaError> {
    ExternalCandidateFeatures::new(
        ValidatedCandidateSummary {
            candidate: candidate.identity(),
            predicted_cost_delta: candidate.predicted_cost_delta(),
        },
        contract.required_analyses(),
        candidate.consumed_facts(),
    )
}
