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
pub const SELECTED_LOWERING_RULE_CATALOG: [SelectedLoweringRuleCatalogEntry; 15] = [
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
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingLiteralCopyMaterialization,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::COPY_V1,
            pairs: &[SelectedInstructionPairRule::COPY_LITERAL_FOLD],
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingU12ByteViewAddressOffset,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
            pairs: &[SelectedInstructionPairRule::BYTE_VIEW_ADDRESS_OFFSET_U12],
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingExactDivideIdentityCopy,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::EXACT_DIVIDE_V1,
            pairs: &[SelectedInstructionPairRule::EXACT_DIVIDE_ONE_COPY],
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingWrappingRemainderOneZeroMaterialization,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
            pairs: &[SelectedInstructionPairRule::WRAPPING_REMAINDER_ONE_MATERIALIZE],
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingBitwiseAndZeroMaterialization,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::BITWISE_AND_ZERO_V1,
            pairs: &SelectedInstructionPairRule::BITWISE_AND_ZERO_FOLDS,
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingBitwiseXorZeroIdentityCopy,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::BITWISE_XOR_ZERO_V1,
            pairs: &SelectedInstructionPairRule::BITWISE_XOR_ZERO_COPIES,
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1,
            pairs: &SelectedInstructionPairRule::WRAPPING_ADD_ZERO_COPIES,
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingBitwiseAndOnesIdentityCopy,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::BITWISE_AND_ONES_V1,
            pairs: &SelectedInstructionPairRule::BITWISE_AND_ONES_COPIES,
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingWrappingRemainderZeroDividendZeroMaterialization,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
            pairs: &[SelectedInstructionPairRule::WRAPPING_REMAINDER_ZERO_DIVIDEND_MATERIALIZE],
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingExactDivideZeroDividendZeroMaterialization,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1,
            pairs: &[SelectedInstructionPairRule::EXACT_DIVIDE_ZERO_DIVIDEND_MATERIALIZE],
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
pub const ORDERED_SELECTED_LOWERING_RULES: [Optimization; 15] = [
    SELECTED_LOWERING_RULE_CATALOG[0].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[1].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[2].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[3].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[4].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[5].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[6].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[7].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[8].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[9].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[10].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[11].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[12].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[13].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[14].optimization(),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedLoweringRuleCatalogError {
    WrongPhase(OptimizationPhaseMismatch),
    MissingSelection,
    UnsupportedSelection(Optimization),
}
