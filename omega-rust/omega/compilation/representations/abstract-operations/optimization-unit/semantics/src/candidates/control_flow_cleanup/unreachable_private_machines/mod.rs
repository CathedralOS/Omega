//! Optimizer module role: executable entrance. Unreachable-machine validation.
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod replay;

pub(crate) fn validate_unreachable_private_machines_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::super::contract::admit(
        input,
        candidate,
        &super::super::contract::UNREACHABLE_PRIVATE_MACHINE_PRUNING,
    )?;
    if !candidate.affected_blocks().is_empty() || !candidate.substitutions().is_empty() {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    replay::validate(input, candidate)
}
