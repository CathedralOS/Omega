//! Optimizer module role: executable entrance. Dead scalar-node candidate validation coordination.
//!
//! This entrance admits candidate custody and the analysis contract. Exact
//! rule classification, the exhaustive operation partition, and independent
//! rewrite replay descend into named leaves.
use super::contract;
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_core::AnalysisKind;
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod operation_partition;
mod rule_catalog;
mod validation;

/// Independently validate and apply one dead scalar-node elimination.
pub(crate) fn validate_dead_scalar_node_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    contract::admit_origin(input, candidate)?;
    let safety_class = rule_catalog::expected_safety(candidate.rule())
        .ok_or(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)?;
    contract::admit_roster(
        candidate,
        &[AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries],
        contract::USE_DEFINITION_AND_EFFECT_SUMMARIES,
        safety_class,
        Some(-1),
    )?;
    validation::validate_dead_scalar_node_candidate(input, candidate)
}
