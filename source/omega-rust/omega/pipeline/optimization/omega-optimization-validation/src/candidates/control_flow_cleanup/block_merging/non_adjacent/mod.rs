//! Optimizer module role: executable entrance. Non-adjacent block-merge validation.

use super::*;

mod replay;

/// Independently replay one non-adjacent unique-predecessor block merge. The
/// validator treats source-roster order as serialization only: it reconstructs
/// execution dominance, every global parameter substitution, all moved value
/// definitions, and every dense-effect relocation before total validation.
pub fn validate_non_adjacent_block_merge_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::super::contract::validate(
        input,
        candidate,
        b"omega.psi-rule.non-adjacent-unique-predecessor-block-merge.v1",
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
            AnalysisKind::UseDefinition,
            AnalysisKind::OwnershipFrontiers,
        ]),
        AnalysisInvalidationSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ]),
        OptimizationSafetyClass::StructuralIdentity,
    )?;
    if candidate.predicted_cost_delta() != -2 {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    replay::validate(input, candidate)
}
