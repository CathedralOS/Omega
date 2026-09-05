//! Optimizer module role: executable entrance. Dead scalar-node candidate validation coordination.
//!
//! This entrance admits candidate custody and the analysis contract. Exact
//! rule classification, the exhaustive operation partition, and independent
//! rewrite replay descend into named leaves.

use super::*;

mod operation_partition;
mod rule_catalog;
mod validation;

/// Independently validate and apply one dead scalar-node elimination.
pub fn validate_dead_scalar_node_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_candidate_contract(input, candidate)?;
    validation::validate_dead_scalar_node_candidate(input, candidate)
}

fn validate_candidate_contract(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let expected_safety = rule_catalog::expected_safety(candidate.rule())
        .ok_or(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)?;
    if candidate.required_analyses()
        != AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries])
        || candidate.invalidated_analyses()
            != AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ])
        || candidate.safety_class() != expected_safety
        || candidate.predicted_cost_delta() != -1
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    Ok(())
}
