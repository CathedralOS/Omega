use optimization_core::{Optimization, OptimizationCatalogDescriptor, OptimizationPhaseMismatch};

use super::super::RegisterAllocationRuleTargetApplicability;
use super::{LiteralFoldPolicy, SelectedInstructionPairRule};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedLoweringRuleCatalogPayload {
    target: RegisterAllocationRuleTargetApplicability,
    policy: LiteralFoldPolicy,
    pair: SelectedInstructionPairRule,
}

impl SelectedLoweringRuleCatalogPayload {
    pub const fn target(self) -> RegisterAllocationRuleTargetApplicability {
        self.target
    }

    pub const fn policy(self) -> LiteralFoldPolicy {
        self.policy
    }

    pub const fn pair(self) -> SelectedInstructionPairRule {
        self.pair
    }
}

pub type SelectedLoweringRuleCatalogEntry =
    OptimizationCatalogDescriptor<SelectedLoweringRuleCatalogPayload>;

/// The single selected-lowering enable/order catalog.
pub const SELECTED_LOWERING_RULE_CATALOG: [SelectedLoweringRuleCatalogEntry; 3] = [
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingU12ExactAddImmediate,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::EXACT_ADD_V1,
            pair: SelectedInstructionPairRule::EXACT_ADD_IMMEDIATE_U12,
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingU12ExactSubtractImmediate,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::EXACT_SUBTRACT_V1,
            pair: SelectedInstructionPairRule::EXACT_SUBTRACT_IMMEDIATE_U12,
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingU12CompareImmediate,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::COMPARE_V1,
            pair: SelectedInstructionPairRule::COMPARE_IMMEDIATE_U12,
        },
    ),
];

/// Descriptors of every catalog row enabled by `policy`, in catalog order.
pub fn enabled_pair_rules(
    policy: LiteralFoldPolicy,
) -> impl Iterator<Item = SelectedInstructionPairRule> {
    SELECTED_LOWERING_RULE_CATALOG
        .into_iter()
        .filter(move |entry| policy.contains(entry.payload().policy()))
        .map(|entry| entry.payload().pair())
}

/// Compatibility view derived from the descriptor catalog.
pub const ORDERED_SELECTED_LOWERING_RULES: [Optimization; 3] = [
    SELECTED_LOWERING_RULE_CATALOG[0].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[1].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[2].optimization(),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedLoweringRuleCatalogError {
    WrongPhase(OptimizationPhaseMismatch),
    MissingSelection,
    UnsupportedSelection(Optimization),
}
