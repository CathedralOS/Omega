//! Optimizer module role: executable entrance. Adjacent block-merge validation.
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod replay;

pub(crate) fn validate_adjacent_block_merge_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::super::super::contract::admit(
        input,
        candidate,
        &super::super::super::contract::ADJACENT_BLOCK_MERGE,
    )?;
    replay::validate(input, candidate)
}
