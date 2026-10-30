//! Optimizer module role: executable entrance. Redundant block-parameter validation coordination.
//!
//! This entrance admits candidate custody and the structural-identity analysis
//! contract. Witness reconstruction, closed-region observation, and exhaustive
//! operation rewriting descend into named leaves.
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod observation;
mod operation_rewrite;
mod validation;

pub(crate) use operation_rewrite::rewrite_block_parameter_operation;

/// Independently validate and apply one redundant block-parameter rewrite.
pub(crate) fn validate_redundant_block_parameter_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::contract::admit(
        input,
        candidate,
        &super::contract::REDUNDANT_BLOCK_PARAMETER,
    )?;
    validation::validate_redundant_block_parameter_candidate(input, candidate)
}
