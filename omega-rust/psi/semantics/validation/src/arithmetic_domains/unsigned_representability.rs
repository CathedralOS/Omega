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

#[cfg(test)]
mod tests {
    use super::*;

    fn interval(low: i64, high: i64) -> Interval {
        Interval {
            low: Some(low),
            high: Some(high),
        }
    }

    const FULL: Interval = Interval {
        low: Some(0),
        high: None,
    };

    #[test]
    fn unknown_upper_endpoint_still_has_the_u64_ceiling() {
        assert!(!binary_fits(
            BinaryOperator::Add,
            FULL,
            interval(1, 1),
            None,
            None
        ));
        assert!(binary_fits(
            BinaryOperator::Add,
            FULL,
            interval(0, 0),
            None,
            None
        ));
        assert!(binary_fits(
            BinaryOperator::Multiply,
            FULL,
            interval(1, 1),
            None,
            None
        ));
        assert!(binary_fits(
            BinaryOperator::Multiply,
            FULL,
            interval(0, 0),
            None,
            None
        ));
        assert!(!binary_fits(
            BinaryOperator::Multiply,
            FULL,
            interval(2, 2),
            None,
            None
        ));
    }

    #[test]
    fn exact_constants_recover_representability_above_i64_maximum() {
        assert!(binary_fits(
            BinaryOperator::Add,
            interval(i64::MAX, i64::MAX),
            interval(1, 1),
            None,
            None
        ));
        assert!(binary_fits(
            BinaryOperator::Add,
            FULL,
            interval(1, 1),
            Some(u64::MAX - 1),
            None
        ));
        assert!(!binary_fits(
            BinaryOperator::Add,
            FULL,
            interval(1, 1),
            Some(u64::MAX),
            None
        ));
        assert!(binary_fits(
            BinaryOperator::Multiply,
            FULL,
            interval(1, 1),
            Some(u64::MAX),
            None
        ));
        assert!(!binary_fits(
            BinaryOperator::Multiply,
            FULL,
            interval(2, 2),
            Some(u64::MAX),
            None
        ));
    }

    #[test]
    fn subtraction_requires_the_independent_lower_bound() {
        assert!(binary_fits(
            BinaryOperator::Subtract,
            FULL,
            interval(0, 0),
            None,
            None
        ));
        assert!(!binary_fits(
            BinaryOperator::Subtract,
            FULL,
            interval(1, 1),
            None,
            None
        ));
        assert!(binary_fits(
            BinaryOperator::Subtract,
            Interval {
                low: Some(1),
                high: None
            },
            interval(0, 1),
            None,
            None
        ));
        assert!(binary_fits(
            BinaryOperator::Subtract,
            FULL,
            FULL,
            Some(u64::MAX),
            None
        ));
        assert!(!binary_fits(
            BinaryOperator::Subtract,
            FULL,
            FULL,
            Some(u64::MAX - 1),
            None
        ));
    }

    #[test]
    fn multiplication_uses_mathematical_u128_endpoints() {
        let maximum_u32 = i64::from(u32::MAX);
        assert!(binary_fits(
            BinaryOperator::Multiply,
            interval(0, maximum_u32),
            interval(0, maximum_u32),
            None,
            None
        ));
        assert!(!binary_fits(
            BinaryOperator::Multiply,
            FULL,
            FULL,
            Some(1_u64 << 32),
            Some(1_u64 << 32)
        ));
        assert!(!binary_fits(
            BinaryOperator::Multiply,
            FULL,
            FULL,
            Some(u64::MAX),
            Some(u64::MAX)
        ));
    }

    #[test]
    fn exact_shift_checks_value_overflow_and_the_unmasked_count() {
        assert!(binary_fits(
            BinaryOperator::ShiftLeft,
            FULL,
            interval(0, 0),
            None,
            None
        ));
        assert!(!binary_fits(
            BinaryOperator::ShiftLeft,
            FULL,
            interval(1, 1),
            None,
            None
        ));
        assert!(binary_fits(
            BinaryOperator::ShiftLeft,
            interval(1, 1),
            interval(63, 63),
            None,
            None
        ));
        assert!(!binary_fits(
            BinaryOperator::ShiftLeft,
            interval(2, 2),
            interval(63, 63),
            None,
            None
        ));
        for count in [
            interval(-1, 0),
            interval(64, 64),
            FULL,
            Interval {
                low: None,
                high: Some(3),
            },
        ] {
            assert!(!binary_fits(
                BinaryOperator::ShiftLeft,
                interval(0, 0),
                count,
                None,
                None
            ));
        }
        assert!(binary_fits(
            BinaryOperator::ShiftLeft,
            interval(1, 1),
            FULL,
            None,
            Some(63)
        ));
        assert!(!binary_fits(
            BinaryOperator::ShiftLeft,
            interval(0, 0),
            FULL,
            None,
            Some(64)
        ));
    }

    #[test]
    fn malformed_intervals_do_not_establish_representability() {
        for invalid in [interval(-1, 10), interval(0, -1), interval(10, 9)] {
            for operator in [
                BinaryOperator::Add,
                BinaryOperator::Subtract,
                BinaryOperator::Multiply,
                BinaryOperator::ShiftLeft,
            ] {
                assert!(!binary_fits(operator, invalid, interval(0, 0), None, None));
                assert!(!binary_fits(operator, interval(0, 0), invalid, None, None));
            }
        }
        assert!(!binary_fits(
            BinaryOperator::BitwiseAnd,
            FULL,
            FULL,
            None,
            None
        ));
    }
}
