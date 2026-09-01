//! Optimizer module role: executable entrance. Shared terminal-jump validation.

use super::*;

mod replay;

pub fn validate_shared_jump_fusion_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::contract::validate(
        input,
        candidate,
        b"omega.psi-rule.shared-terminal-jump-fusion.v2",
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::OwnershipFrontiers,
            AnalysisKind::PostDominators,
        ]),
        AnalysisInvalidationSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ]),
        OptimizationSafetyClass::StructuralIdentity,
    )?;
    if candidate.predicted_cost_delta() != -1 {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    replay::validate(input, candidate)
}
