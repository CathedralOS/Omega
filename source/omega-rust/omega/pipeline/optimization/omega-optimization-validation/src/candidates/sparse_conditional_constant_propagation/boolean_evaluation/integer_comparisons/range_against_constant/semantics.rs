//! Independent range/literal rule classification and interval evaluation.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::OptimizationRuleIdentity;
use psi_core::{IntegerType, IntegerValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidatedIntegerRangeComparisonKind {
    RangeEqualConstant,
    ConstantEqualRange,
    RangeLessThanConstant,
    ConstantLessThanRange,
    RangeLessOrEqualConstant,
    ConstantLessOrEqualRange,
}

pub(crate) fn classify(
    rule: OptimizationRuleIdentity,
    operation: &O,
) -> Option<ValidatedIntegerRangeComparisonKind> {
    let kind = if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-equal-range-constant.v1",
        ) {
        ValidatedIntegerRangeComparisonKind::RangeEqualConstant
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-equal-constant-range.v1",
        )
    {
        ValidatedIntegerRangeComparisonKind::ConstantEqualRange
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-than-range-constant.v1",
        )
    {
        ValidatedIntegerRangeComparisonKind::RangeLessThanConstant
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-than-constant-range.v1",
        )
    {
        ValidatedIntegerRangeComparisonKind::ConstantLessThanRange
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-or-equal-range-constant.v1",
        )
    {
        ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-or-equal-constant-range.v1",
        )
    {
        ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange
    } else {
        return None;
    };
    match (kind, operation) {
        (
            ValidatedIntegerRangeComparisonKind::RangeEqualConstant
            | ValidatedIntegerRangeComparisonKind::ConstantEqualRange,
            O::IntegerEqual { .. },
        )
        | (
            ValidatedIntegerRangeComparisonKind::RangeLessThanConstant
            | ValidatedIntegerRangeComparisonKind::ConstantLessThanRange,
            O::IntegerLessThan { .. },
        )
        | (
            ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant
            | ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange,
            O::IntegerLessOrEqual { .. },
        ) => Some(kind),
        _ => None,
    }
}

pub(crate) fn evaluate(
    kind: ValidatedIntegerRangeComparisonKind,
    scalar_type: IntegerType,
    minimum: IntegerValue,
    maximum: IntegerValue,
    constant: IntegerValue,
) -> Option<bool> {
    let minimum_to_constant = scalar_type.compare(minimum, constant)?;
    let maximum_to_constant = scalar_type.compare(maximum, constant)?;
    match kind {
        ValidatedIntegerRangeComparisonKind::RangeEqualConstant
        | ValidatedIntegerRangeComparisonKind::ConstantEqualRange => {
            if minimum_to_constant.is_eq() && maximum_to_constant.is_eq() {
                Some(true)
            } else if minimum_to_constant.is_gt() || maximum_to_constant.is_lt() {
                Some(false)
            } else {
                None
            }
        }
        ValidatedIntegerRangeComparisonKind::RangeLessThanConstant => maximum_to_constant
            .is_lt()
            .then_some(true)
            .or_else(|| (!minimum_to_constant.is_lt()).then_some(false)),
        ValidatedIntegerRangeComparisonKind::ConstantLessThanRange => minimum_to_constant
            .is_gt()
            .then_some(true)
            .or_else(|| (!maximum_to_constant.is_gt()).then_some(false)),
        ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant => (!maximum_to_constant
            .is_gt())
        .then_some(true)
        .or_else(|| minimum_to_constant.is_gt().then_some(false)),
        ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange => (!minimum_to_constant
            .is_lt())
        .then_some(true)
        .or_else(|| maximum_to_constant.is_lt().then_some(false)),
    }
}
