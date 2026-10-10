//! Optimizer module role: executable entrance. Field-value specialization validation.
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod replay;

pub fn validate_field_value_specialization_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::contract::validate(
        input,
        candidate,
        b"omega.psi-rule.field-value-specialization.v1",
        AnalysisSet::new([AnalysisKind::StronglyConnectedComponents]),
        AnalysisInvalidationSet::new([
            AnalysisKind::UseDefinition,
            AnalysisKind::ScalarConstants,
            AnalysisKind::ValueRanges,
            AnalysisKind::EffectSummaries,
        ]),
        OptimizationSafetyClass::StructuralIdentity,
    )?;
    replay::validate(input, candidate)
}
