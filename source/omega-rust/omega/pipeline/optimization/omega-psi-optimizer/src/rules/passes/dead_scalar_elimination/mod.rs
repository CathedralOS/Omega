//! Removal of unused scalar computations, grouped by their semantic safety proof.

mod catalog;

pub(in crate::rules) use catalog::built_in_registrations;

use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};

use super::{DEAD_PURE_SCALAR_PASS_NAME, PROOF_CHECK_ELISION_PASS_NAME};

mod accounting;
mod proposal;
mod rules;
mod shapes;

pub use rules::{
    DeadScalarLiteralEliminationRule, DeadUnconditionallyTotalScalarEliminationRule,
    ProofCertifiedDeadScalarEliminationRule,
};

/// The three closed safety arguments under which unused scalar work may leave
/// the verified unit. This is the family catalog and coordination point: shape
/// mechanics, proposal construction, and custody accounting live below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeadScalarFamily {
    Literal,
    UnconditionallyTotal,
    ProofCertified,
}

impl DeadScalarFamily {
    fn contract(self, rule_name: &'static [u8]) -> OptimizationRuleContract {
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
