//! Optimizer module role: executable entrance. Redundant block-parameter validation coordination.
//!
//! This entrance admits candidate custody and the structural-identity analysis
//! contract. Witness reconstruction, closed-region observation, and exhaustive
//! operation rewriting descend into named leaves.
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod observation;
mod operation_rewrite;
mod validation;

pub(crate) use operation_rewrite::rewrite_block_parameter_operation;

/// Independently validate and apply one redundant block-parameter rewrite.
pub(crate) fn validate_redundant_block_parameter_candidate(
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
    super::contract::validate(
        input,
        candidate,
        b"omega.psi-rule.redundant-block-parameter.v1",
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
            AnalysisKind::UseDefinition,
        ]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        OptimizationSafetyClass::StructuralIdentity,
    )?;
    if candidate.predicted_cost_delta() != -1 {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    Ok(())
}
