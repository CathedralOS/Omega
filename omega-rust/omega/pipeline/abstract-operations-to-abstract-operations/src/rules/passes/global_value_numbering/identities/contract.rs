//! Shared contract shape for exact total-scalar identity rules.

use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};

use super::super::super::GLOBAL_VALUE_NUMBERING_PASS_NAME;

const REQUIRED_ANALYSES: [AnalysisKind; 3] = [
    AnalysisKind::ScalarConstants,
    AnalysisKind::UseDefinition,
    AnalysisKind::EffectSummaries,
];

const INVALIDATED_ANALYSES: [AnalysisKind; 2] =
    [AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries];

pub(super) fn exact_total_scalar_identity(identity: &[u8]) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(identity),
        OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME),
        1,
        AnalysisSet::new(REQUIRED_ANALYSES),
        AnalysisInvalidationSet::new(INVALIDATED_ANALYSES),
        OptimizationSafetyClass::ExactOperationSemantics,
    )
    .expect("built-in rule has nonzero version")
}
