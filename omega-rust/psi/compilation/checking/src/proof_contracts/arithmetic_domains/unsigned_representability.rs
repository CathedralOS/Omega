//! Exact u64 representability beyond the i64 interval endpoint window.

use super::{BinaryOperator, Interval};

/// Check the mathematical result against the real u64 carrier ceiling.
///
/// The caller establishes that value operands are u64 results, including
/// recursively validating the expressions that produced them. An absent
/// endpoint can then use that carrier bound; it is not mathematical infinity.
/// Exact known values preserve information lost by the i64 interval projection.
/// This query supplies no relational proof between independent operands.
pub(super) fn binary_fits(
    operator: BinaryOperator,
    left: Interval,
    right: Interval,
    left_known: Option<u64>,
    right_known: Option<u64>,
) -> bool {
    let Some((left_low, left_high)) = value_bounds(left, left_known) else {
        return false;
    };
    // A shift's count may have a signed carrier. Do not replace its missing
    // lower endpoint with the u64 value carrier's zero floor.
    if operator == BinaryOperator::ShiftLeft && right_known.is_none() && right.low.is_none() {
        return false;
    }
    let Some((right_low, right_high)) = value_bounds(right, right_known) else {
        return false;
    };
    let maximum = u128::from(u64::MAX);
    match operator {
        BinaryOperator::Add => left_high
            .checked_add(right_high)
            .is_some_and(|high| high <= maximum),
        BinaryOperator::Subtract => left_low >= right_high,
        BinaryOperator::Multiply => left_high
            .checked_mul(right_high)
            .is_some_and(|high| high <= maximum),
        BinaryOperator::ShiftLeft => {
            // Counts are already nonnegative from value_bounds. Exact shifts
            // never adopt the machine instruction's modulo count behavior.
            if right_low >= 64 || right_high >= 64 {
                return false;
            }
            left_high
                .checked_shl(right_high as u32)
                .is_some_and(|high| high <= maximum)
        }
        _ => false,
    }
}

fn value_bounds(interval: Interval, known: Option<u64>) -> Option<(u128, u128)> {
    if let Some(known) = known {
        let known = u128::from(known);
        return Some((known, known));
    }
    let low = match interval.low {
        Some(low) => u128::try_from(low).ok()?,
        None => 0,
    };
    let high = match interval.high {
        Some(high) => u128::try_from(high).ok()?,
        None => u128::from(u64::MAX),
    };
    (low <= high && high <= u128::from(u64::MAX)).then_some((low, high))
}
