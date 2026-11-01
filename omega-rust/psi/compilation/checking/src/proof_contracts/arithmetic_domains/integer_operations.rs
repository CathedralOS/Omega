//! Shared mathematical integer interval operations.
//!
//! Proof obligations and source checks use this domain rather than carrying
//! separate endpoint arithmetic. Bounds describe values only: selected operator
//! meaning, operand formation, policy and occurrence availability remain with
//! the calling judgment. IntegerRange is durable evidence vocabulary, not a
//! second interval engine.

use super::BigIntInterval;
use facts::IntegerRange;
use numerics::bignum::BigInt;
use typed_trees::expression::BinaryOperator;

pub fn integer_binary_range(
    operator: BinaryOperator,
    left: IntegerRange,
    right: IntegerRange,
) -> Option<IntegerRange> {
    if left.minimum > left.maximum || right.minimum > right.maximum {
        return None;
    }
    let first = BigIntInterval::from_closed(left.clone());
    let second = BigIntInterval::from_closed(right.clone());
    match operator {
        BinaryOperator::Add => first.add(&second).into_closed(),
        BinaryOperator::Subtract => first.subtract(&second).into_closed(),
        BinaryOperator::Multiply => first.multiply(&second).into_closed(),
        BinaryOperator::Divide if second.excludes_zero() => first.divide(&second).into_closed(),
        BinaryOperator::Modulo if second.excludes_zero() => first.remainder(&second).into_closed(),
        BinaryOperator::ShiftRight => first
            .shift_right(right.minimum.to_u64()?, right.maximum.to_u64()?)
            .into_closed(),
        BinaryOperator::BitwiseAnd
            if !left.minimum.is_negative() && !right.minimum.is_negative() =>
        {
            Some(IntegerRange {
                minimum: BigInt::zero(),
                maximum: left.maximum.min(right.maximum),
            })
        }
        BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor
            if !left.minimum.is_negative() && !right.minimum.is_negative() =>
        {
            let bits = left.maximum.clone().max(right.maximum).bit_length();
            Some(IntegerRange {
                minimum: if operator == BinaryOperator::BitwiseOr {
                    left.minimum.max(right.minimum)
                } else {
                    BigInt::zero()
                },
                maximum: BigInt::from_i64(1).shl_bits(bits).sub(&BigInt::from_i64(1)),
            })
        }
        _ => None,
    }
}

/// Fold the source clients' existing exact i64 scalar arithmetic through the
/// shared arbitrary-precision interval operation. This introduces no new
/// selected operator, and a result beyond the caller's carrier remains unknown.
pub fn folded_integer_binary(left: i64, operator: BinaryOperator, right: i64) -> Option<i64> {
    if !matches!(
        operator,
        BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
    ) {
        return None;
    }
    let point = |value| IntegerRange {
        minimum: BigInt::from_i64(value),
        maximum: BigInt::from_i64(value),
    };
    let left_value = BigInt::from_i64(left);
    let right_value = BigInt::from_i64(right);
    if matches!(operator, BinaryOperator::Divide | BinaryOperator::Modulo) {
        let (quotient, remainder) = left_value.div_rem(&right_value)?;
        // Exact i64 remainder shares division definedness at MIN / -1.
        quotient.to_i64()?;
        return if operator == BinaryOperator::Divide {
            quotient.to_i64()
        } else {
            remainder.to_i64()
        };
    }
    let result = integer_binary_range(operator, point(left), point(right))?;
    (result.minimum == result.maximum)
        .then(|| result.minimum.to_i64())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closed(low: i64, high: i64) -> IntegerRange {
        IntegerRange {
            minimum: BigInt::from_i64(low),
            maximum: BigInt::from_i64(high),
        }
    }

    #[test]
    fn integer_interval_operations_enclose_small_models() {
        // Enumerate concrete values independently of the interval formulas.
        // Both dividend signs and both divisor signs must be represented.
        for low in -3..=3 {
            for high in low..=3 {
                for other_low in -3..=3 {
                    for other_high in other_low..=3 {
                        for operator in [
                            BinaryOperator::Add,
                            BinaryOperator::Subtract,
                            BinaryOperator::Multiply,
                            BinaryOperator::Divide,
                            BinaryOperator::Modulo,
                        ] {
                            if matches!(operator, BinaryOperator::Divide | BinaryOperator::Modulo)
                                && other_low <= 0
                                && other_high >= 0
                            {
                                assert_eq!(
                                    integer_binary_range(
                                        operator,
                                        closed(low, high),
                                        closed(other_low, other_high)
                                    ),
                                    None
                                );
                                continue;
                            }
                            let bounds = integer_binary_range(
                                operator,
                                closed(low, high),
                                closed(other_low, other_high),
                            )
                            .unwrap();
                            for left in low..=high {
                                for right in other_low..=other_high {
                                    let value = match operator {
                                        BinaryOperator::Add => left + right,
                                        BinaryOperator::Subtract => left - right,
                                        BinaryOperator::Multiply => left * right,
                                        BinaryOperator::Divide => left / right,
                                        BinaryOperator::Modulo => left % right,
                                        _ => unreachable!("enumerated arithmetic operator"),
                                    };
                                    let value = BigInt::from_i64(value);
                                    assert!(
                                        bounds.minimum <= value && value <= bounds.maximum,
                                        "{operator:?} over [{low},{high}] and [{other_low},{other_high}] excludes {value:?}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn integer_intervals_preserve_full_width_and_unbounded_edges() {
        let maximum = BigInt::from_u64(u64::MAX);
        let full_unsigned = BigIntInterval {
            low: Some(BigInt::zero()),
            high: Some(maximum.clone()),
        };
        assert_eq!(
            full_unsigned.divide(&BigIntInterval::constant(BigInt::from_i64(1))),
            full_unsigned
        );
        assert_eq!(
            full_unsigned
                .divide(&BigIntInterval::constant(BigInt::from_i64(2)))
                .high,
            Some(BigInt::from_u64(u64::MAX / 2))
        );
        assert_eq!(
            BigIntInterval::unbounded().multiply(&BigIntInterval::constant(BigInt::zero())),
            BigIntInterval::constant(BigInt::zero())
        );

        let signed = BigIntInterval::from_closed(closed(i64::MIN, i64::MAX));
        let remainder = signed.remainder(&BigIntInterval::constant(BigInt::from_i64(i64::MIN)));
        assert_eq!(remainder.into_closed(), Some(closed(-i64::MAX, i64::MAX)));

        assert_eq!(
            integer_binary_range(BinaryOperator::ShiftRight, closed(-3, -3), closed(1, 1)),
            Some(closed(-2, -2))
        );
        assert_eq!(
            super::super::floor_shift_right(&BigInt::from_i64(-3), u64::MAX),
            Some(BigInt::from_i64(-1))
        );
        assert_eq!(
            super::super::floor_shift_right(&maximum, u64::MAX),
            Some(BigInt::zero())
        );
        assert_eq!(
            folded_integer_binary(i64::MIN, BinaryOperator::Divide, -1),
            None
        );
        assert_eq!(
            folded_integer_binary(i64::MIN, BinaryOperator::Modulo, -1),
            None
        );
        assert_eq!(folded_integer_binary(1, BinaryOperator::Divide, 0), None);
    }
}
