//! Optimizer module role: executable entrance. Adjacent block-merge validation.
use crate::AnalysisInvalidationSet;
use crate::AnalysisKind;
use crate::AnalysisSet;
use crate::OptimizationSafetyClass;
use crate::OptimizationUnitValidationError;
use crate::PsiOptimizationUnit;
use crate::PsiRewriteCandidate;
use crate::ValidatedPsiRewrite;

mod replay;

pub fn validate_adjacent_block_merge_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::super::contract::validate(
        input,
        candidate,
        b"omega.psi-rule.adjacent-single-predecessor-block-merge.v5",
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
