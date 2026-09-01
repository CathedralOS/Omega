//! Shared independent admission for exact control-flow rule contracts.

use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisSet, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use omega_optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};

pub(super) fn validate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
    rule_domain: &'static [u8],
    required_analyses: AnalysisSet,
    invalidated_analyses: AnalysisInvalidationSet,
    safety_class: OptimizationSafetyClass,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if candidate.rule() != OptimizationRuleIdentity::from_canonical_bytes(rule_domain)
        || candidate.required_analyses() != required_analyses
        || candidate.invalidated_analyses() != invalidated_analyses
        || candidate.safety_class() != safety_class
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    Ok(())
}
