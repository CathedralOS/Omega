//! Optimizer module role: executable entrance. Case-membership specialization validation.
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet,
    OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod replay;

pub fn validate_case_membership_specialization_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::contract::validate(
        input,
        candidate,
        b"omega.psi-rule.case-membership-specialization.v1",
        AnalysisSet::new([AnalysisKind::StronglyConnectedComponents]),
        AnalysisInvalidationSet::new([
            AnalysisKind::UseDefinition,
            AnalysisKind::ScalarConstants,
            AnalysisKind::ValueRanges,
            AnalysisKind::EffectSummaries,
        ]),
        OptimizationSafetyClass::StructuralIdentity,
    )?;
    if !candidate.substitutions().is_empty() {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    replay::validate(input, candidate)
}
