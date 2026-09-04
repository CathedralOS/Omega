//! Optimizer module role: executable entrance. Unreachable-machine validation.

use super::*;

mod replay;

pub fn validate_unreachable_private_machines_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::contract::validate(
        input,
        candidate,
        b"omega.psi-rule.unreachable-private-machine-pruning.v1",
        AnalysisSet::new([AnalysisKind::CallGraph]),
        AnalysisInvalidationSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::CallGraph,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ]),
        OptimizationSafetyClass::StructuralIdentity,
    )?;
    if !candidate.affected_blocks().is_empty() || !candidate.substitutions().is_empty() {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    replay::validate(input, candidate)
}
