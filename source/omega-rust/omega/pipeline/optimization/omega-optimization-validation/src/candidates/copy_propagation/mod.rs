//! Optimizer module role: executable entrance. Redundant block-parameter validation coordination.
//!
//! This entrance admits candidate custody and the structural-identity analysis
//! contract. Witness reconstruction, closed-region observation, and exhaustive
//! operation rewriting descend into named leaves.

use super::*;

mod observation;
mod operation_rewrite;
mod validation;

#[cfg(test)]
pub(crate) use observation::{
    normalize_redundant_parameter_observation_input, unchanged_outside_redundant_parameter_region,
};
pub(crate) use operation_rewrite::rewrite_block_parameter_operation;

/// Independently validate and apply one redundant block-parameter rewrite.
pub fn validate_redundant_block_parameter_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_candidate_contract(input, candidate)?;
    validation::validate_redundant_block_parameter_candidate(input, candidate)
}

fn validate_candidate_contract(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if candidate.rule()
        != OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.redundant-block-parameter.v1",
        )
        || candidate.required_analyses()
            != AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
            ])
        || candidate.invalidated_analyses()
            != AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ])
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
        || candidate.predicted_cost_delta() != -1
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    Ok(())
}
