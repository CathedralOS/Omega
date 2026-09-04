//! Optimizer module role: executable entrance. Linear empty-block validation.

use super::*;

mod replay;

pub fn validate_linear_empty_block_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::super::contract::validate(
        input,
        candidate,
        b"omega.psi-rule.linear-empty-block-thread.v2",
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
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
    if candidate.predicted_cost_delta() != -3 || !candidate.substitutions().is_empty() {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    replay::validate(input, candidate)
}
