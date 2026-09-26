//! Optimizer module role: executable entrance. Global value numbering: the
//! pass's exact rule order, one file per rule beneath it -- same-block,
//! dominator and phi-translated common-subexpression elimination in their
//! total, proof-certified and compatible-policy forms, and the seven
//! obligation-free total-scalar identity rules. The three CSE contracts and
//! the exact effect admission every rule consults live here;
//! `expression_keys` owns the canonical expression identities,
//! `total_scalar_identities` the identity rules' shared traversal, and
//! `phi_translated_accounting` the join-parameter provenance accounting.

use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{BlockId, MachineId};

use crate::EffectSummaryAnalysis;
use crate::rules::GLOBAL_VALUE_NUMBERING_PASS_NAME;
use crate::rules::catalog::BuiltInRuleRegistration;

mod bitwise_absorbing_literal_identity;
mod bitwise_neutral_literal_identity;
mod dominator_proof_certified_compatible_policy_scalar_gvn;
mod dominator_proof_certified_scalar_gvn;
mod dominator_total_scalar_gvn;
mod expression_keys;
mod phi_translated_accounting;
mod phi_translated_obligation_free_scalar_gvn;
mod phi_translated_proof_certified_compatible_policy_scalar_gvn;
mod phi_translated_proof_certified_scalar_gvn;
mod same_block_proof_certified_compatible_policy_scalar_cse;
mod same_block_proof_certified_scalar_cse;
mod same_block_total_scalar_cse;
mod saturating_multiply_zero_annihilation;
mod saturating_neutral_arithmetic_identity;
mod total_scalar_identities;
mod wrapping_multiply_zero_annihilation;
mod wrapping_neutral_arithmetic_identity;
mod wrapping_shift_zero_count_identity;

pub use bitwise_absorbing_literal_identity::BitwiseAbsorbingLiteralIdentityRule;
pub use bitwise_neutral_literal_identity::BitwiseNeutralLiteralIdentityRule;
pub use dominator_proof_certified_compatible_policy_scalar_gvn::DominatorProofCertifiedCompatiblePolicyScalarGvnRule;
pub use dominator_proof_certified_scalar_gvn::DominatorProofCertifiedScalarGvnRule;
pub use dominator_total_scalar_gvn::DominatorTotalScalarGvnRule;
pub use phi_translated_obligation_free_scalar_gvn::PhiTranslatedObligationFreeScalarGvnRule;
pub use phi_translated_proof_certified_compatible_policy_scalar_gvn::PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule;
pub use phi_translated_proof_certified_scalar_gvn::PhiTranslatedProofCertifiedScalarGvnRule;
pub use same_block_proof_certified_compatible_policy_scalar_cse::SameBlockProofCertifiedCompatiblePolicyScalarCseRule;
pub use same_block_proof_certified_scalar_cse::SameBlockProofCertifiedScalarCseRule;
pub use same_block_total_scalar_cse::SameBlockTotalScalarCseRule;
pub use saturating_multiply_zero_annihilation::SaturatingMultiplyZeroAnnihilationRule;
pub use saturating_neutral_arithmetic_identity::SaturatingNeutralArithmeticIdentityRule;
pub use wrapping_multiply_zero_annihilation::WrappingMultiplyZeroAnnihilationRule;
pub use wrapping_neutral_arithmetic_identity::WrappingNeutralArithmeticIdentityRule;
pub use wrapping_shift_zero_count_identity::WrappingShiftZeroCountIdentityRule;

#[cfg(test)]
pub(super) use expression_keys::{
    compatible_policy_scalar_leader, compatible_policy_scalar_redundant,
    proof_certified_scalar_expression,
};

/// The exact local rule order for this pass.
pub(super) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, SameBlockTotalScalarCseRule),
        BuiltInRuleRegistration::new(1, SameBlockProofCertifiedScalarCseRule),
        BuiltInRuleRegistration::new(2, DominatorTotalScalarGvnRule),
        BuiltInRuleRegistration::new(3, DominatorProofCertifiedScalarGvnRule),
        BuiltInRuleRegistration::new(4, PhiTranslatedObligationFreeScalarGvnRule),
        BuiltInRuleRegistration::new(5, PhiTranslatedProofCertifiedScalarGvnRule),
        BuiltInRuleRegistration::new(6, SameBlockProofCertifiedCompatiblePolicyScalarCseRule),
        BuiltInRuleRegistration::new(7, DominatorProofCertifiedCompatiblePolicyScalarGvnRule),
        BuiltInRuleRegistration::new(8, PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule),
        BuiltInRuleRegistration::new(9, WrappingNeutralArithmeticIdentityRule),
        BuiltInRuleRegistration::new(10, WrappingShiftZeroCountIdentityRule),
        BuiltInRuleRegistration::new(11, WrappingMultiplyZeroAnnihilationRule),
        BuiltInRuleRegistration::new(12, SaturatingNeutralArithmeticIdentityRule),
        BuiltInRuleRegistration::new(13, SaturatingMultiplyZeroAnnihilationRule),
        BuiltInRuleRegistration::new(14, BitwiseNeutralLiteralIdentityRule),
        BuiltInRuleRegistration::new(15, BitwiseAbsorbingLiteralIdentityRule),
    ]
}

pub(super) fn same_block_contract(
    rule_name: &[u8],
    safety: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(rule_name),
        OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME),
        1,
        AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        safety,
    )
    .expect("built-in rule has nonzero version")
}

pub(super) fn dominating_contract(
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

pub(super) fn phi_translated_contract(
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

pub(super) fn exact_pure_scalar_effect(
    unit: &PsiOptimizationUnit,
    effects: &EffectSummaryAnalysis,
    machine: MachineId,
    block: BlockId,
    node: u32,
) -> bool {
    effects.nodes.iter().any(|row| {
        row.revision == unit.identity
            && row.machine == machine
            && row.block == block
            && row.node == node
            && row.class == crate::EffectClass::PureScalar
            && row.observable == crate::EffectKnowledge::No
            && row.structural_state == crate::EffectKnowledge::No
            && row.crash == crate::EffectKnowledge::No
            && row.suspension == crate::EffectKnowledge::No
    })
}
