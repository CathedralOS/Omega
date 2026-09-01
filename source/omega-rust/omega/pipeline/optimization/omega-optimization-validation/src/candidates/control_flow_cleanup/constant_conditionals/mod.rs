//! Optimizer module role: executable entrance. Constant-conditional validation.

use super::*;

mod replay;

pub fn validate_constant_conditional_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::contract::validate(
        input,
        candidate,
        b"omega.psi-rule.constant-conditional-fold.v5",
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::ScalarConstants,
        ]),
        AnalysisInvalidationSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::CallGraph,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ]),
        OptimizationSafetyClass::ExactOperationSemantics,
    )?;
    if candidate.predicted_cost_delta() != -1 || !candidate.substitutions().is_empty() {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    replay::validate(input, candidate)
}
