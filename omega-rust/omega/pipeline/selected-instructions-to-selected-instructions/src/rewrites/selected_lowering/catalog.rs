use optimization_core::{Optimization, OptimizationCatalogDescriptor, OptimizationPhaseMismatch};

use super::super::RegisterAllocationRuleTargetApplicability;
use super::{LiteralFoldPolicy, SelectedInstructionPairRule};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedLoweringRuleCatalogPayload {
    target: RegisterAllocationRuleTargetApplicability,
    policy: LiteralFoldPolicy,
    pairs: &'static [SelectedInstructionPairRule],
}

impl SelectedLoweringRuleCatalogPayload {
    pub const fn target(self) -> RegisterAllocationRuleTargetApplicability {
        self.target
    }

    pub const fn policy(self) -> LiteralFoldPolicy {
        self.policy
    }

    /// Every symbolic instruction-pair rule this exact selection admits, in
    /// catalog order. A selection names one family; a family may cover several
    /// disjoint consumer kinds, as the extension-elimination selection does.
    pub const fn pairs(self) -> &'static [SelectedInstructionPairRule] {
        self.pairs
    }
}

pub type SelectedLoweringRuleCatalogEntry =
    OptimizationCatalogDescriptor<SelectedLoweringRuleCatalogPayload>;

/// The single selected-lowering enable/order catalog.
pub const SELECTED_LOWERING_RULE_CATALOG: [SelectedLoweringRuleCatalogEntry; 5] = [
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingU12ExactAddImmediate,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::EXACT_ADD_V1,
            pairs: &[
                SelectedInstructionPairRule::EXACT_ADD_IMMEDIATE_U12,
                SelectedInstructionPairRule::EXACT_ADD_LEFT_IMMEDIATE_U12,
            ],
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingU12ExactSubtractImmediate,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::EXACT_SUBTRACT_V1,
            pairs: &[SelectedInstructionPairRule::EXACT_SUBTRACT_IMMEDIATE_U12],
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingU12CompareImmediate,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::COMPARE_V1,
            pairs: &[SelectedInstructionPairRule::COMPARE_IMMEDIATE_U12],
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingLiteralExtensionElimination,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::EXTENSION_V1,
            pairs: &SelectedInstructionPairRule::EXTENSION_LITERAL_FOLDS,
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingU12Load8IndexedOffset,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::LOAD8_INDEXED_V1,
            pairs: &[SelectedInstructionPairRule::LOAD8_INDEXED_U12],
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
        .flat_map(|entry| entry.payload().pairs().iter().copied())
}

/// Compatibility view derived from the descriptor catalog.
pub const ORDERED_SELECTED_LOWERING_RULES: [Optimization; 5] = [
    SELECTED_LOWERING_RULE_CATALOG[0].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[1].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[2].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[3].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[4].optimization(),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedLoweringRuleCatalogError {
    WrongPhase(OptimizationPhaseMismatch),
    MissingSelection,
    UnsupportedSelection(Optimization),
}
