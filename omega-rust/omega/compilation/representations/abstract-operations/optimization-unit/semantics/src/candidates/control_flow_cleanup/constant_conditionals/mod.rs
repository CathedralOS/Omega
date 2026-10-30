//! Optimizer module role: executable entrance. Constant-conditional validation.
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod replay;

pub(crate) fn validate_constant_conditional_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::super::contract::admit(
        input,
        candidate,
        &super::super::contract::CONSTANT_CONDITIONAL_FOLD,
    )?;
    if !candidate.substitutions().is_empty() {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    replay::validate(input, candidate)
}
