//! Optimizer module role: executable entrance. Field-value specialization validation.
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod replay;

pub fn validate_field_value_specialization_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::contract::admit(
        input,
        candidate,
        &super::contract::FIELD_VALUE_SPECIALIZATION,
    )?;
    replay::validate(input, candidate)
}
