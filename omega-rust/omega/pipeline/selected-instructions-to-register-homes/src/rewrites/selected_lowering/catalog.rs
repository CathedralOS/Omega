use optimization_core::{Optimization, OptimizationCatalogDescriptor, OptimizationPhaseMismatch};

use super::super::RegisterAllocationRuleTargetApplicability;
use super::LiteralFoldPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedLoweringRuleCatalogPayload {
    target: RegisterAllocationRuleTargetApplicability,
    policy: LiteralFoldPolicy,
}

impl SelectedLoweringRuleCatalogPayload {
    pub const fn target(self) -> RegisterAllocationRuleTargetApplicability {
        self.target
    }

    pub const fn policy(self) -> LiteralFoldPolicy {
        self.policy
    }
}

pub type SelectedLoweringRuleCatalogEntry =
    OptimizationCatalogDescriptor<SelectedLoweringRuleCatalogPayload>;

/// The single selected-lowering enable/order catalog.
pub const SELECTED_LOWERING_RULE_CATALOG: [SelectedLoweringRuleCatalogEntry; 2] = [
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingU12ExactAddImmediate,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::EXACT_ADD_V1,
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingU12ExactSubtractImmediate,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        },
    ),
];

/// Compatibility view derived from the descriptor catalog.
pub const ORDERED_SELECTED_LOWERING_RULES: [Optimization; 2] = [
    SELECTED_LOWERING_RULE_CATALOG[0].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[1].optimization(),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedLoweringRuleCatalogError {
    WrongPhase(OptimizationPhaseMismatch),
    MissingSelection,
    UnsupportedSelection(Optimization),
}
