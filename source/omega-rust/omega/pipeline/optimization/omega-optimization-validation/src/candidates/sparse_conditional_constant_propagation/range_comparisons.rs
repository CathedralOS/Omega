//! Independent typed-range comparison classification and evaluation.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidatedIntegerRangeComparisonKind {
    RangeEqualConstant,
    ConstantEqualRange,
    RangeLessThanConstant,
    ConstantLessThanRange,
    RangeLessOrEqualConstant,
    ConstantLessOrEqualRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidatedIntegerRangePairComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}

pub(crate) fn independently_validated_integer_range_pair_comparison_kind(
    rule: OptimizationRuleIdentity,
    operation: &O,
) -> Option<ValidatedIntegerRangePairComparisonKind> {
    let kind = if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-equal-range-range.v1",
        ) {
        ValidatedIntegerRangePairComparisonKind::Equal
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-than-range-range.v1",
        )
    {
        ValidatedIntegerRangePairComparisonKind::LessThan
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-or-equal-range-range.v1",
        )
    {
        ValidatedIntegerRangePairComparisonKind::LessOrEqual
    } else {
        return None;
    };
    match (kind, operation) {
        (ValidatedIntegerRangePairComparisonKind::Equal, O::IntegerEqual { .. })
        | (ValidatedIntegerRangePairComparisonKind::LessThan, O::IntegerLessThan { .. })
        | (ValidatedIntegerRangePairComparisonKind::LessOrEqual, O::IntegerLessOrEqual { .. }) => {
            Some(kind)
        }
        _ => None,
    }
}

pub(crate) fn independently_evaluate_integer_range_pair_comparison(
    kind: ValidatedIntegerRangePairComparisonKind,
    scalar_type: psi_core::IntegerType,
    same_value: bool,
    left_minimum: psi_core::IntegerValue,
    left_maximum: psi_core::IntegerValue,
    right_minimum: psi_core::IntegerValue,
    right_maximum: psi_core::IntegerValue,
) -> Option<bool> {
    if same_value {
        return Some(!matches!(
            kind,
            ValidatedIntegerRangePairComparisonKind::LessThan
        ));
    }
    let left_maximum_to_right_minimum = scalar_type.compare(left_maximum, right_minimum)?;
    let left_minimum_to_right_maximum = scalar_type.compare(left_minimum, right_maximum)?;
    match kind {
        ValidatedIntegerRangePairComparisonKind::Equal => {
            let both_equal_singletons = scalar_type.compare(left_minimum, left_maximum)?.is_eq()
                && scalar_type.compare(right_minimum, right_maximum)?.is_eq()
                && scalar_type.compare(left_minimum, right_minimum)?.is_eq();
            both_equal_singletons.then_some(true).or_else(|| {
                (left_maximum_to_right_minimum.is_lt() || left_minimum_to_right_maximum.is_gt())
                    .then_some(false)
            })
        }
        ValidatedIntegerRangePairComparisonKind::LessThan => left_maximum_to_right_minimum
            .is_lt()
            .then_some(true)
            .or_else(|| (!left_minimum_to_right_maximum.is_lt()).then_some(false)),
        ValidatedIntegerRangePairComparisonKind::LessOrEqual => (!left_maximum_to_right_minimum
            .is_gt())
        .then_some(true)
        .or_else(|| left_minimum_to_right_maximum.is_gt().then_some(false)),
    }
}

pub(crate) fn independently_validated_integer_range_comparison_kind(
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

pub(crate) fn independently_evaluate_integer_range_comparison(
    kind: ValidatedIntegerRangeComparisonKind,
    scalar_type: psi_core::IntegerType,
    minimum: psi_core::IntegerValue,
    maximum: psi_core::IntegerValue,
    constant: psi_core::IntegerValue,
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
