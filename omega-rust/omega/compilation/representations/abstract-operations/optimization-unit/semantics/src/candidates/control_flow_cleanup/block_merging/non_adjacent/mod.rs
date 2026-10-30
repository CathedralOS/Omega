//! Optimizer module role: executable entrance. Non-adjacent block-merge validation.
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod replay;

/// Independently replay one non-adjacent unique-predecessor block merge. The
/// validator treats source-roster order as serialization only: it reconstructs
/// execution dominance, every global parameter substitution, all moved value
/// definitions, and every dense-effect relocation before total validation.
pub(crate) fn validate_non_adjacent_block_merge_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::super::super::contract::admit(
        input,
        candidate,
        &super::super::super::contract::NON_ADJACENT_BLOCK_MERGE,
    )?;
    replay::validate(input, candidate)
}
