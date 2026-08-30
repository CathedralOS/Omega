//! Optimizer module role: executable entrance. Same-block common-subexpression rules.
//!
//! The three exact rule families share one analysis/invalidation contract but
//! keep their expression and evidence mechanics in named leaves.

mod compatible_policy;
mod obligation_free;
mod proof_certified;

use super::*;

pub use compatible_policy::SameBlockProofCertifiedCompatiblePolicyScalarCseRule;
pub use obligation_free::SameBlockTotalScalarCseRule;
pub use proof_certified::SameBlockProofCertifiedScalarCseRule;

fn same_block_contract(
    rule_name: &[u8],
    safety: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(rule_name),
        OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME),
        1,
        AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        AnalysisInvalidationSet::new([
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ]),
        safety,
    )
    .expect("built-in rule has nonzero version")
}
