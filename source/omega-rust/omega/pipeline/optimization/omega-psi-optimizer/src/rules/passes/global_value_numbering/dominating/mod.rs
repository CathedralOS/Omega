//! Optimizer module role: stage group. Dominating common-subexpression rule family map.
//!
//! The three exact rule families share one analysis/invalidation contract but
//! keep their expression and evidence mechanics in named leaves.

mod compatible_policy;
mod obligation_free;
mod proof_certified;

use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};

use super::super::GLOBAL_VALUE_NUMBERING_PASS_NAME;

pub use compatible_policy::DominatorProofCertifiedCompatiblePolicyScalarGvnRule;
pub use obligation_free::DominatorTotalScalarGvnRule;
pub use proof_certified::DominatorProofCertifiedScalarGvnRule;

fn dominating_contract(
    rule_name: &[u8],
    safety: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(rule_name),
        OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME),
        1,
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        safety,
    )
    .expect("built-in rule has nonzero version")
}
