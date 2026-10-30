//! Optimizer module role: executable entrance. Shared terminal-jump validation.
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod replay;

pub(crate) fn validate_shared_jump_fusion_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::super::contract::admit(
        input,
        candidate,
        &super::super::contract::SHARED_TERMINAL_JUMP_FUSION,
    )?;
    replay::validate(input, candidate)
}
