//! Optimizer module role: executable entrance. Field-value specialization validation.
use crate::AnalysisInvalidationSet;
use crate::AnalysisKind;
use crate::AnalysisSet;
use crate::OptimizationRuleIdentity;
use crate::OptimizationSafetyClass;
use crate::OptimizationUnitValidationError;
use crate::PsiOptimizationUnit;
use crate::PsiRewriteCandidate;
use crate::ValidatedPsiRewrite;
use crate::validate_psi_optimization_unit;

mod replay;

pub fn validate_field_value_specialization_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if candidate.rule()
        != OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.field-value-specialization.v1",
        )
        || candidate.required_analyses()
            != AnalysisSet::new([AnalysisKind::StronglyConnectedComponents])
        || candidate.invalidated_analyses()
            != AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::ScalarConstants,
                AnalysisKind::ValueRanges,
                AnalysisKind::EffectSummaries,
            ])
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    replay::validate(input, candidate)
}
