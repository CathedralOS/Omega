//! Independent range-pair rule classification and interval evaluation.

use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationRuleIdentity;
use semantic_vocabulary::{IntegerType, IntegerValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidatedIntegerRangePairComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}

pub(crate) fn classify(
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

pub(crate) fn evaluate(
    kind: ValidatedIntegerRangePairComparisonKind,
    scalar_type: IntegerType,
    same_value: bool,
    left_minimum: IntegerValue,
    left_maximum: IntegerValue,
    right_minimum: IntegerValue,
    right_maximum: IntegerValue,
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
