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
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ValueLiveness)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != rule_catalog::expected_safety(candidate.rule())
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    Ok(())
}
