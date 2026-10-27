//! The obligation-free total-scalar identity rule family: one table of rule
//! rows over `total_scalar_identities`' shared traversal, each row naming its
//! canonical identity bytes and the closed law partition it selects.

use abstract_operations::{
    AbstractOperation as O, IntegerOverflowPolicy, IntegerShiftDirection, IntegerShiftPolicy,
};
use optimization_core::OptimizationRuleContract;
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate, TotalScalarIdentityKind};
use semantic_vocabulary::{IntegerType, IntegerValue, OperationId, ValueId};

use crate::rules::global_value_numbering::total_scalar_identities::{
    NeutralArithmeticKinds, TotalScalarIdentityShape, all_ones, exact_total_scalar_identity,
    multiply_zero_annihilation_shapes, neutral_arithmetic_shapes, propose_total_scalar_identities,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

/// One obligation-free total-scalar identity rule: its type, canonical
/// identity bytes, and the closed law partition it proposes over. Every rule
/// of the family shares [`exact_total_scalar_identity`]'s contract and
/// [`propose_total_scalar_identities`]'s traversal; only the partition differs.
macro_rules! total_scalar_identity_rules {
    ($($(#[$meta:meta])* $name:ident = ($identity:literal, $classify:path);)*) => {$(
        psi_rule_contract!(
            $(#[$meta])*
            $name,
            exact_total_scalar_identity($identity),
            propose = propose_total_scalar_identities,
            $classify,
        );
    )*};
}

total_scalar_identity_rules! {
    /// The exact wrapping neutral-arithmetic rule, and the closed law partition it proposes over.
    WrappingNeutralArithmeticIdentityRule = (
        b"omega.psi-rule.live-obligation-free-wrapping-integer-neutral-arithmetic-identity-elimination.v1",
        wrapping_neutral_arithmetic_classify
    );
    /// The exact saturating neutral-arithmetic rule, and the closed law partition it proposes over.
    SaturatingNeutralArithmeticIdentityRule = (
        b"omega.psi-rule.live-obligation-free-saturating-integer-neutral-arithmetic-identity-elimination.v1",
        saturating_neutral_arithmetic_classify
    );
    /// The exact wrapping shift-zero-count rule, and the closed law partition it proposes over.
    WrappingShiftZeroCountIdentityRule = (
        b"omega.psi-rule.live-obligation-free-wrapping-integer-shift-zero-count-elimination.v1",
        wrapping_shift_zero_count_classify
    );
    /// The exact wrapping multiply-zero rule, and the closed law partition it proposes over.
    WrappingMultiplyZeroAnnihilationRule = (
        b"omega.psi-rule.live-obligation-free-wrapping-integer-multiply-zero-annihilation.v1",
        wrapping_multiply_zero_annihilation_classify
    );
    /// The exact saturating multiply-zero rule, and the closed law partition it proposes over.
    SaturatingMultiplyZeroAnnihilationRule = (
        b"omega.psi-rule.live-obligation-free-saturating-integer-multiply-zero-annihilation.v1",
        saturating_multiply_zero_annihilation_classify
    );
    /// The exact bitwise neutral-literal rule, and the closed law partition it proposes over.
    BitwiseNeutralLiteralIdentityRule = (
        b"omega.psi-rule.live-obligation-free-integer-bitwise-neutral-literal-elimination.v1",
        bitwise_neutral_literal_classify
    );
    /// The exact bitwise absorbing-literal rule, and the closed law partition it proposes over.
    BitwiseAbsorbingLiteralIdentityRule = (
        b"omega.psi-rule.live-obligation-free-integer-bitwise-absorbing-literal-elimination.v1",
        bitwise_absorbing_literal_classify
    );
}
pub(super) fn wrapping_neutral_arithmetic_classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    use TotalScalarIdentityKind as Kind;
    neutral_arithmetic_shapes(
        operation,
        IntegerOverflowPolicy::Wrapping,
        NeutralArithmeticKinds {
            add_zero_left: Kind::WrappingIntegerAddZeroLeft,
            add_zero_right: Kind::WrappingIntegerAddZeroRight,
            subtract_zero_right: Kind::WrappingIntegerSubtractZeroRight,
            multiply_one_left: Kind::WrappingIntegerMultiplyOneLeft,
            multiply_one_right: Kind::WrappingIntegerMultiplyOneRight,
        },
    )
}

pub(super) fn saturating_neutral_arithmetic_classify(
    operation: &O,
) -> Vec<TotalScalarIdentityShape> {
    use TotalScalarIdentityKind as Kind;
    neutral_arithmetic_shapes(
        operation,
        IntegerOverflowPolicy::Saturating,
        NeutralArithmeticKinds {
            add_zero_left: Kind::SaturatingIntegerAddZeroLeft,
            add_zero_right: Kind::SaturatingIntegerAddZeroRight,
            subtract_zero_right: Kind::SaturatingIntegerSubtractZeroRight,
            multiply_one_left: Kind::SaturatingIntegerMultiplyOneLeft,
            multiply_one_right: Kind::SaturatingIntegerMultiplyOneRight,
        },
    )
}

pub(super) fn wrapping_shift_zero_count_classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    let Some(operands) = operation.integer_shift() else {
        return Vec::new();
    };
    if !matches!(operands.policy, IntegerShiftPolicy::Wrapping) {
        return Vec::new();
    }
    let identity = match operands.direction {
        IntegerShiftDirection::Left => TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount,
        IntegerShiftDirection::Right => TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount,
    };
    vec![TotalScalarIdentityShape {
        source_operation: operands.psi_operation,
        result: operands.result,
        replacement: operands.value,
        law_operand: operands.count,
        scalar_type: operands.value_type,
        law_operand_type: operands.count_type,
        identity,
        expected_law_value: operands.count_type.zero(),
    }]
}

pub(super) fn wrapping_multiply_zero_annihilation_classify(
    operation: &O,
) -> Vec<TotalScalarIdentityShape> {
    multiply_zero_annihilation_shapes(
        operation,
        IntegerOverflowPolicy::Wrapping,
        TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft,
        TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight,
    )
}

pub(super) fn saturating_multiply_zero_annihilation_classify(
    operation: &O,
) -> Vec<TotalScalarIdentityShape> {
    multiply_zero_annihilation_shapes(
        operation,
        IntegerOverflowPolicy::Saturating,
        TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft,
        TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight,
    )
}

pub(super) fn bitwise_neutral_literal_classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    match operation {
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => bitwise_neutral_pair(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft,
            TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight,
            all_ones(*scalar_type),
        ),
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => bitwise_neutral_pair(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft,
            TotalScalarIdentityKind::IntegerBitwiseOrZeroRight,
            scalar_type.zero(),
        ),
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => bitwise_neutral_pair(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft,
            TotalScalarIdentityKind::IntegerBitwiseXorZeroRight,
            scalar_type.zero(),
        ),
        _ => Vec::new(),
    }
}

fn bitwise_neutral_pair(
    source_operation: OperationId,
    result: ValueId,
    scalar_type: IntegerType,
    left: ValueId,
    right: ValueId,
    left_identity: TotalScalarIdentityKind,
    right_identity: TotalScalarIdentityKind,
    law_value: IntegerValue,
) -> Vec<TotalScalarIdentityShape> {
    vec![
        TotalScalarIdentityShape {
            source_operation,
            result,
            replacement: right,
            law_operand: left,
            scalar_type,
            law_operand_type: scalar_type,
            identity: left_identity,
            expected_law_value: law_value,
        },
        TotalScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            law_operand: right,
            scalar_type,
            law_operand_type: scalar_type,
            identity: right_identity,
            expected_law_value: law_value,
        },
    ]
}

pub(super) fn bitwise_absorbing_literal_classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    match operation {
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => bitwise_absorbing_pair(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft,
            TotalScalarIdentityKind::IntegerBitwiseAndZeroRight,
            scalar_type.zero(),
        ),
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => bitwise_absorbing_pair(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft,
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight,
            all_ones(*scalar_type),
        ),
        _ => Vec::new(),
    }
}

fn bitwise_absorbing_pair(
    source_operation: OperationId,
    result: ValueId,
    scalar_type: IntegerType,
    left: ValueId,
    right: ValueId,
    left_identity: TotalScalarIdentityKind,
    right_identity: TotalScalarIdentityKind,
    law_value: IntegerValue,
) -> Vec<TotalScalarIdentityShape> {
    vec![
        TotalScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            law_operand: left,
            scalar_type,
            law_operand_type: scalar_type,
            identity: left_identity,
            expected_law_value: law_value,
        },
        TotalScalarIdentityShape {
            source_operation,
            result,
            replacement: right,
            law_operand: right,
            scalar_type,
            law_operand_type: scalar_type,
            identity: right_identity,
            expected_law_value: law_value,
        },
    ]
}
