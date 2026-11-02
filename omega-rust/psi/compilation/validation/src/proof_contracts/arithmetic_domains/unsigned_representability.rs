//! Full-width u64 bounds and Exact representability beyond the i64 window.

use super::{BinaryOperator, Interval};

/// Quotient bounds for already-landed u64 operands. Open compatibility ends
/// use the real carrier extent, and exact unsigned snapshots retain wide
/// constants instead of turning them into a weaker signed-window floor.
///
/// With a positive divisor, unsigned truncated division increases with the
/// dividend and decreases with the divisor. Opposite endpoint pairs therefore
/// give both extrema. A possible zero divisor supplies no result bound; neither
/// a tighter quotient nor carrier completion discharges operand formation.
pub(super) fn divide_interval(
    left: Interval,
    right: Interval,
    left_known: Option<u64>,
    right_known: Option<u64>,
) -> Interval {
    let (Some((left_low, left_high)), Some((right_low, right_high))) = (
        value_bounds(left, left_known),
        value_bounds(right, right_known),
    ) else {
        return Interval::UNBOUNDED;
    };
    if right_low == 0 {
        return Interval::UNBOUNDED;
    }
    let low = left_low / right_high;
    let high = left_high / right_low;
    Interval {
        // A wide floor still proves this weaker finite floor. Its ceiling
        // remains open when the exact u64 value cannot fit the i64 window.
        low: Some(i64::try_from(low).unwrap_or(i64::MAX)),
        high: i64::try_from(high).ok(),
    }
}

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
