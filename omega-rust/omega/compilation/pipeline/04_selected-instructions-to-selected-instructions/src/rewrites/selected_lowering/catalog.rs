use optimization_core::{Optimization, OptimizationCatalogDescriptor, OptimizationPhaseMismatch};

use super::super::RegisterAllocationRuleTargetApplicability;
use super::{LiteralFoldPolicy, SelectedInstructionPairRule};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectedLoweringRuleCatalogPayload {
    target: RegisterAllocationRuleTargetApplicability,
    policy: LiteralFoldPolicy,
    pairs: &'static [SelectedInstructionPairRule],
}

impl SelectedLoweringRuleCatalogPayload {
    receipt_accessor_rows!(SelectedLoweringRuleCatalogPayload {
        policy: LiteralFoldPolicy,
    /// Every symbolic instruction-pair rule this exact selection admits, in
    /// catalog order. A selection names one family; a family may cover several
    /// disjoint consumer kinds, as the extension-elimination selection does.
        pairs: &'static [SelectedInstructionPairRule],
    });
}

pub(crate) type SelectedLoweringRuleCatalogEntry =
    OptimizationCatalogDescriptor<SelectedLoweringRuleCatalogPayload>;

/// One selected-lowering catalog row. Every family in this catalog is
/// target-independent; the payload still records that applicability.
const fn row(
    optimization: Optimization,
    policy: LiteralFoldPolicy,
    pairs: &'static [SelectedInstructionPairRule],
) -> SelectedLoweringRuleCatalogEntry {
    SelectedLoweringRuleCatalogEntry::new(
        optimization,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy,
            pairs,
        },
    )
}

/// The single selected-lowering enable/order catalog.
pub(crate) const SELECTED_LOWERING_RULE_CATALOG: [SelectedLoweringRuleCatalogEntry; 23] = [
    row(
        Optimization::SelectedIncomingU12ExactAddImmediate,
        LiteralFoldPolicy::EXACT_ADD_V1,
        &[
            SelectedInstructionPairRule::EXACT_ADD_IMMEDIATE_U12,
            SelectedInstructionPairRule::EXACT_ADD_LEFT_IMMEDIATE_U12,
        ],
    ),
    row(
        Optimization::SelectedIncomingU12ExactSubtractImmediate,
        LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        &[SelectedInstructionPairRule::EXACT_SUBTRACT_IMMEDIATE_U12],
    ),
    row(
        Optimization::SelectedIncomingU12CompareImmediate,
        LiteralFoldPolicy::COMPARE_V1,
        &[
            SelectedInstructionPairRule::COMPARE_IMMEDIATE_U12,
            SelectedInstructionPairRule::COMPARE_LEFT_IMMEDIATE_U12,
            SelectedInstructionPairRule::COMPARE_ZERO,
            SelectedInstructionPairRule::COMPARE_LEFT_ZERO,
        ],
    ),
    row(
        Optimization::SelectedIncomingLiteralExtensionElimination,
        LiteralFoldPolicy::EXTENSION_V1,
        &SelectedInstructionPairRule::EXTENSION_LITERAL_FOLDS,
    ),
    row(
        Optimization::SelectedIncomingU12Load8IndexedOffset,
        LiteralFoldPolicy::LOAD8_INDEXED_V1,
        &[SelectedInstructionPairRule::LOAD8_INDEXED_U12],
    ),
    row(
        Optimization::SelectedIncomingLiteralCopyMaterialization,
        LiteralFoldPolicy::COPY_V1,
        &[SelectedInstructionPairRule::COPY_LITERAL_FOLD],
    ),
    row(
        Optimization::SelectedIncomingU12ByteViewAddressOffset,
        LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        &[
            SelectedInstructionPairRule::BYTE_VIEW_ADDRESS_OFFSET_U12,
            SelectedInstructionPairRule::BYTE_VIEW_ADDRESS_BACKING_U12,
        ],
    ),
    row(
        Optimization::SelectedIncomingExactDivideIdentityCopy,
        LiteralFoldPolicy::EXACT_DIVIDE_V1,
        &[SelectedInstructionPairRule::EXACT_DIVIDE_ONE_COPY],
    ),
    row(
        Optimization::SelectedIncomingWrappingRemainderOneZeroMaterialization,
        LiteralFoldPolicy::WRAPPING_REMAINDER_V1,
        &[SelectedInstructionPairRule::WRAPPING_REMAINDER_ONE_MATERIALIZE],
    ),
    row(
        Optimization::SelectedIncomingBitwiseAndZeroMaterialization,
        LiteralFoldPolicy::BITWISE_AND_ZERO_V1,
        &SelectedInstructionPairRule::BITWISE_AND_ZERO_FOLDS,
    ),
    row(
        Optimization::SelectedIncomingBitwiseXorZeroIdentityCopy,
        LiteralFoldPolicy::BITWISE_XOR_ZERO_V1,
        &SelectedInstructionPairRule::BITWISE_XOR_ZERO_COPIES,
    ),
    row(
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1,
        &SelectedInstructionPairRule::WRAPPING_ADD_ZERO_COPIES,
    ),
    row(
        Optimization::SelectedIncomingBitwiseAndOnesIdentityCopy,
        LiteralFoldPolicy::BITWISE_AND_ONES_V1,
        &SelectedInstructionPairRule::BITWISE_AND_ONES_COPIES,
    ),
    row(
        Optimization::SelectedIncomingWrappingRemainderZeroDividendZeroMaterialization,
        LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1,
        &[SelectedInstructionPairRule::WRAPPING_REMAINDER_ZERO_DIVIDEND_MATERIALIZE],
    ),
    row(
        Optimization::SelectedIncomingExactDivideZeroDividendZeroMaterialization,
        LiteralFoldPolicy::EXACT_DIVIDE_ZERO_V1,
        &[SelectedInstructionPairRule::EXACT_DIVIDE_ZERO_DIVIDEND_MATERIALIZE],
    ),
    row(
        Optimization::SelectedIncomingSaturatingAddZeroIdentityCopy,
        LiteralFoldPolicy::SATURATING_ADD_ZERO_V1,
        &SelectedInstructionPairRule::SATURATING_ADD_ZERO_COPIES,
    ),
    row(
        Optimization::SelectedIncomingSaturatingSubtractZeroIdentityCopy,
        LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
        &SelectedInstructionPairRule::SATURATING_SUBTRACT_ZERO_COPIES,
    ),
    row(
        Optimization::SelectedIncomingSaturatingDivideOneIdentityCopy,
        LiteralFoldPolicy::SATURATING_DIVIDE_ONE_V1,
        &SelectedInstructionPairRule::SATURATING_DIVIDE_ONE_COPIES,
    ),
    row(
        Optimization::SelectedIncomingSaturatingDivideZeroDividendZeroMaterialization,
        LiteralFoldPolicy::SATURATING_DIVIDE_ZERO_V1,
        &SelectedInstructionPairRule::SATURATING_DIVIDE_ZERO_DIVIDEND_MATERIALIZATIONS,
    ),
    row(
        Optimization::SelectedIncomingSaturatingSubtractZeroMinuendZeroMaterialization,
        LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_MINUEND_V1,
        &SelectedInstructionPairRule::SATURATING_SUBTRACT_ZERO_MINUEND_MATERIALIZATIONS,
    ),
    row(
        Optimization::SelectedIncomingSaturatingAddUpperBoundMaterialization,
        LiteralFoldPolicy::SATURATING_ADD_UPPER_BOUND_V1,
        &SelectedInstructionPairRule::SATURATING_ADD_UPPER_BOUND_MATERIALIZATIONS,
    ),
    row(
        Optimization::SelectedIncomingWrappingRemainderMinusOneZeroMaterialization,
        LiteralFoldPolicy::WRAPPING_REMAINDER_MINUS_ONE_V1,
        &[SelectedInstructionPairRule::WRAPPING_REMAINDER_MINUS_ONE_MATERIALIZE],
    ),
    row(
        Optimization::SelectedIncomingSaturatingSubtractUpperBoundSubtrahendZeroMaterialization,
        LiteralFoldPolicy::SATURATING_SUBTRACT_UPPER_BOUND_V1,
        &SelectedInstructionPairRule::SATURATING_SUBTRACT_UPPER_BOUND_MATERIALIZATIONS,
    ),
];

/// Descriptors of every catalog row enabled by `policy`, in catalog order.
pub(crate) fn enabled_pair_rules(
    policy: LiteralFoldPolicy,
) -> impl Iterator<Item = SelectedInstructionPairRule> {
    SELECTED_LOWERING_RULE_CATALOG
        .into_iter()
        .filter(move |entry| policy.contains(entry.payload().policy()))
        .flat_map(|entry| entry.payload().pairs().iter().copied())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectedLoweringRuleCatalogError {
    WrongPhase(OptimizationPhaseMismatch),
    MissingSelection,
    UnsupportedSelection(Optimization),
}
