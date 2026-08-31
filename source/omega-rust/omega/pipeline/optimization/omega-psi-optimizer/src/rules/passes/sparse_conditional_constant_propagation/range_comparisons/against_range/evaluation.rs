use psi_core::{IntegerType, IntegerValue};

use super::IntegerRangePairComparisonKind;

#[allow(clippy::too_many_arguments)]
pub(in crate::rules::passes) fn evaluate(
    kind: IntegerRangePairComparisonKind,
    scalar_type: IntegerType,
    same_value: bool,
    left_minimum: IntegerValue,
    left_maximum: IntegerValue,
    right_minimum: IntegerValue,
    right_maximum: IntegerValue,
) -> Option<bool> {
    if same_value {
        return Some(!matches!(kind, IntegerRangePairComparisonKind::LessThan));
    }
    let left_maximum_to_right_minimum = scalar_type.compare(left_maximum, right_minimum)?;
    let left_minimum_to_right_maximum = scalar_type.compare(left_minimum, right_maximum)?;
    match kind {
        IntegerRangePairComparisonKind::Equal => {
            let both_equal_singletons = scalar_type.compare(left_minimum, left_maximum)?.is_eq()
                && scalar_type.compare(right_minimum, right_maximum)?.is_eq()
                && scalar_type.compare(left_minimum, right_minimum)?.is_eq();
            both_equal_singletons.then_some(true).or_else(|| {
                (left_maximum_to_right_minimum.is_lt() || left_minimum_to_right_maximum.is_gt())
                    .then_some(false)
            })
        }
        IntegerRangePairComparisonKind::LessThan => left_maximum_to_right_minimum
            .is_lt()
            .then_some(true)
            .or_else(|| (!left_minimum_to_right_maximum.is_lt()).then_some(false)),
        IntegerRangePairComparisonKind::LessOrEqual => (!left_maximum_to_right_minimum.is_gt())
            .then_some(true)
            .or_else(|| left_minimum_to_right_maximum.is_gt().then_some(false)),
    }
}
