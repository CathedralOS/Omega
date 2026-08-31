//! Closed semantic safety families shared by the exact dead-scalar rules.

use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};

use super::super::{DEAD_PURE_SCALAR_PASS_NAME, PROOF_CHECK_ELISION_PASS_NAME};

/// The three safety arguments under which unused scalar work may leave a
/// verified optimization unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DeadScalarFamily {
    Literal,
    UnconditionallyTotal,
    ProofCertified,
}

impl DeadScalarFamily {
    pub(super) fn contract(self, rule_name: &'static [u8]) -> OptimizationRuleContract {
        let (pass_name, safety) = match self {
            Self::Literal | Self::UnconditionallyTotal => (
                DEAD_PURE_SCALAR_PASS_NAME,
                OptimizationSafetyClass::ExactOperationSemantics,
            ),
            Self::ProofCertified => (
                PROOF_CHECK_ELISION_PASS_NAME,
                OptimizationSafetyClass::ProofCertified,
            ),
        };
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(rule_name),
            OptimizationPassIdentity::from_canonical_bytes(pass_name),
            1,
            AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            safety,
        )
        .expect("built-in rule has nonzero version")
    }
}
