//! Carrier-aware representation operations; no arithmetic policy is selected.
//!
//! A range determines a common bit prefix, not bitwise endpoint monotonicity.
//! Transfer those known bits and take their signed/unsigned numeric hull.

use super::{BinaryOperator, Interval, PrimitiveType};
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    integer_bit_width, primitive_range,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

pub(crate) fn binary(
    operator: BinaryOperator,
    primitive: PrimitiveType,
    left: Interval,
    right: Interval,
    left_known: Option<u64>,
    right_known: Option<u64>,
) -> Interval {
    let Some(carrier) = Carrier::new(primitive) else {
        return Interval::UNBOUNDED;
    };
    let left = carrier.value_bounds(left, left_known);
    let right = carrier.value_bounds(right, right_known);
    let bounds = match operator {
        BinaryOperator::BitwiseAnd => carrier.integer.bitwise_and_bounds(left, right),
        BinaryOperator::BitwiseOr => carrier.integer.bitwise_or_bounds(left, right),
        BinaryOperator::BitwiseXor => carrier.integer.bitwise_xor_bounds(left, right),
        _ => None,
    };
    bounds.map_or_else(
        || primitive_range(primitive).unwrap_or(Interval::UNBOUNDED),
        |(low, high)| project(value_number(low), value_number(high)),
    )
}

/// A completed right shift uses the operand carrier's finite bounds, including
/// unsigned ceilings outside the interval endpoint window. Count formation is
/// checked by the expression owner; a count that may select modular behavior
/// keeps the complete carrier instead of assuming an ordinary shift.
pub(super) fn shift_right(
    primitive: PrimitiveType,
    value: Interval,
    count: Interval,
    known: Option<u64>,
) -> Interval {
    let Some(carrier) = Carrier::new(primitive) else {
        return Interval::UNBOUNDED;
    };
    let complete = primitive_range(primitive).unwrap_or(Interval::UNBOUNDED);
    let (Some(count_low), Some(count_high)) = (count.low, count.high) else {
        return complete;
    };
    let Some(width) = integer_bit_width(primitive) else {
        return complete;
    };
    if count_low < 0 || count_low > count_high || count_high >= width {
        return complete;
    }
    // The count interval is numeric evidence after the expression owner's
    // source-carrier checks. Its nonnegative endpoints fit this shared kernel
    // count carrier; no source count is reinterpreted or given a new policy.
    let Ok(count_type) = IntegerType::new(IntegerSign::Unsigned, 64) else {
        return complete;
    };
    carrier
        .integer
        .exact_shift_right_bounds(
            carrier.value_bounds(value, known),
            count_type,
            (
                IntegerValue::Unsigned(count_low as u128),
                IntegerValue::Unsigned(count_high as u128),
            ),
        )
        .map_or(complete, |(low, high)| {
            project(value_number(low), value_number(high))
        })
}

pub(super) fn complement(
    primitive: PrimitiveType,
    interval: Interval,
    known: Option<u64>,
) -> Interval {
    let Some(carrier) = Carrier::new(primitive) else {
        return Interval::UNBOUNDED;
    };
    if carrier.integer.sign() == IntegerSign::Unsigned
        && let Some(value) = known
    {
        return complement_unsigned_value(primitive, value).map_or_else(
            || primitive_range(primitive).unwrap_or(Interval::UNBOUNDED),
            |value| project(i128::from(value), i128::from(value)),
        );
    }
    let (input_low, input_high) = carrier.bounds(interval);
    // Fixed-width complement reverses numeric order for both signed and
    // unsigned carriers. Use the shared value evaluator for both endpoints.
    let low = carrier.integer.bitwise_not(carrier.value(input_high));
    let high = carrier.integer.bitwise_not(carrier.value(input_low));
    // The input endpoints are admitted by bounds(); do not manufacture a
    // narrower interval if a future evaluator rejects an unsupported carrier.
    match (low, high) {
        (Some(low), Some(high)) => project(value_number(low), value_number(high)),
        _ => primitive_range(primitive).unwrap_or(Interval::UNBOUNDED),
    }
}

/// Preserve exact unsigned constants outside the i64 interval window.
pub(super) fn binary_unsigned_value(
    operator: BinaryOperator,
    primitive: PrimitiveType,
    left: u64,
    right: u64,
) -> Option<u64> {
    let carrier = Carrier::new(primitive)?;
    if carrier.integer.sign() != IntegerSign::Unsigned {
        return None;
    }
    let (left, right) = (
        IntegerValue::Unsigned(left.into()),
        IntegerValue::Unsigned(right.into()),
    );
    let value = match operator {
        BinaryOperator::BitwiseAnd => carrier.integer.bitwise_and(left, right),
        BinaryOperator::BitwiseOr => carrier.integer.bitwise_or(left, right),
        BinaryOperator::BitwiseXor => carrier.integer.bitwise_xor(left, right),
        _ => None,
    }?;
    let IntegerValue::Unsigned(value) = value else {
        return None;
    };
    u64::try_from(value).ok()
}

pub(super) fn complement_unsigned_value(primitive: PrimitiveType, value: u64) -> Option<u64> {
    let carrier = Carrier::new(primitive)?;
    if carrier.integer.sign() != IntegerSign::Unsigned {
        return None;
    }
    let value = carrier
        .integer
        .bitwise_not(IntegerValue::Unsigned(value.into()))?;
    let IntegerValue::Unsigned(value) = value else {
        return None;
    };
    u64::try_from(value).ok()
}

struct Carrier {
    integer: IntegerType,
}

impl Carrier {
    fn new(primitive: PrimitiveType) -> Option<Self> {
        let width = u16::try_from(integer_bit_width(primitive)?).ok()?;
        let sign = if primitive.is_signed_integer() {
            IntegerSign::Signed
        } else {
            IntegerSign::Unsigned
        };
        let integer = IntegerType::new(sign, width).ok()?;
        Some(Self { integer })
    }

    fn bounds(&self, interval: Interval) -> (i128, i128) {
        let minimum = value_number(self.integer.minimum_value());
        let maximum = value_number(self.integer.maximum_value());
        let low = interval.low.map_or(minimum, i128::from);
        let high = interval.high.map_or(maximum, i128::from);
        if low < minimum || high > maximum || low > high {
            // Non-Exact arithmetic can leave a mathematical overflow interval.
            // Intersecting it with the carrier would falsely describe the
            // wrapped/clamped runtime value. Forget that interval instead.
            (minimum, maximum)
        } else {
            (low, high)
        }
    }

    fn value_bounds(&self, interval: Interval, known: Option<u64>) -> (IntegerValue, IntegerValue) {
        if self.integer.sign() == IntegerSign::Unsigned
            && let Some(value) = known.map(|value| IntegerValue::Unsigned(u128::from(value)))
            && self.integer.admits(value)
        {
            return (value, value);
        }
        let (low, high) = self.bounds(interval);
        (self.value(low), self.value(high))
    }

    fn value(&self, number: i128) -> IntegerValue {
        match self.integer.sign() {
            IntegerSign::Signed => IntegerValue::Signed(number),
            IntegerSign::Unsigned => IntegerValue::Unsigned(number as u128),
        }
    }
}

fn value_number(value: IntegerValue) -> i128 {
    match value {
        IntegerValue::Signed(value) => value,
        // Carrier::new admits source widths <= 64, never an arbitrary u128.
        IntegerValue::Unsigned(value) => value as i128,
    }
}

fn project(low: i128, high: i128) -> Interval {
    Interval {
        // A u64 floor above i64::MAX still proves this weaker finite floor.
        // Its ceiling is unknown in this representation, not i64::MAX.
        low: Some(i64::try_from(low).unwrap_or(i64::MAX)),
        high: i64::try_from(high).ok(),
    }
}
