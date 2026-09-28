//! Carrier-aware representation operations; no arithmetic policy is selected.
//!
//! A range determines a common bit prefix, not bitwise endpoint monotonicity.
//! Transfer those known bits and take their signed/unsigned numeric hull.

use super::{BinaryOperator, Interval, PrimitiveType};
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    integer_bit_width, primitive_range,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

pub(super) fn binary(
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
    let left = carrier.known_bits(left, left_known);
    let right = carrier.known_bits(right, right_known);
    let result = match operator {
        BinaryOperator::BitwiseAnd => KnownBits {
            ones: left.ones & right.ones,
            zeros: left.zeros | right.zeros,
        },
        BinaryOperator::BitwiseOr => KnownBits {
            ones: left.ones | right.ones,
            zeros: left.zeros & right.zeros,
        },
        BinaryOperator::BitwiseXor => KnownBits {
            ones: (left.ones & right.zeros) | (left.zeros & right.ones),
            zeros: (left.ones & right.ones) | (left.zeros & right.zeros),
        },
        _ => return primitive_range(primitive).unwrap_or(Interval::UNBOUNDED),
    };
    carrier.hull(result)
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

#[derive(Clone, Copy)]
struct KnownBits {
    ones: u128,
    zeros: u128,
}

struct Carrier {
    integer: IntegerType,
    mask: u128,
    sign_bit: u128,
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
        Some(Self {
            integer,
            // Source integer carriers are at most 64 bits, so these shifts
            // remain strictly within the temporary u128 bit-pattern window.
            mask: (1_u128 << width) - 1,
            sign_bit: 1_u128 << (width - 1),
        })
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

    fn known_bits(&self, interval: Interval, known: Option<u64>) -> KnownBits {
        if self.integer.sign() == IntegerSign::Unsigned
            && let Some(value) = known
        {
            let bits = u128::from(value);
            return if self.integer.admits(IntegerValue::Unsigned(bits)) {
                KnownBits {
                    ones: bits,
                    zeros: !bits & self.mask,
                }
            } else {
                KnownBits { ones: 0, zeros: 0 }
            };
        }
        let (low, high) = self.bounds(interval);
        if low < 0 && high >= 0 {
            return KnownBits { ones: 0, zeros: 0 };
        }
        let low = (low as u128) & self.mask;
        let high = (high as u128) & self.mask;
        let difference = low ^ high;
        let variable = if difference == 0 {
            0
        } else {
            (1_u128 << (u128::BITS - difference.leading_zeros())) - 1
        };
        let fixed = self.mask & !variable;
        KnownBits {
            ones: low & fixed,
            zeros: !low & fixed,
        }
    }

    fn hull(&self, bits: KnownBits) -> Interval {
        let mut low = bits.ones;
        let mut high = self.mask & !bits.zeros;
        if self.integer.sign() == IntegerSign::Signed
            && (bits.ones | bits.zeros) & self.sign_bit == 0
        {
            // An unknown sign bit gives the most negative admissible pattern
            // as the floor and the greatest positive pattern as the ceiling.
            low |= self.sign_bit;
            high &= !self.sign_bit;
        }
        project(self.number(low), self.number(high))
    }

    fn number(&self, bits: u128) -> i128 {
        if self.integer.sign() == IntegerSign::Signed && bits & self.sign_bit != 0 {
            value_number(self.integer.minimum_value()) + (bits & !self.sign_bit) as i128
        } else {
            bits as i128
        }
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

#[cfg(test)]
mod tests {
    use super::{
        BinaryOperator, Carrier, Interval, PrimitiveType, binary, binary_unsigned_value,
        complement, complement_unsigned_value, project, value_number,
    };

    const OPERATORS: [BinaryOperator; 3] = [
        BinaryOperator::BitwiseAnd,
        BinaryOperator::BitwiseOr,
        BinaryOperator::BitwiseXor,
    ];

    fn interval(low: i64, high: i64) -> Interval {
        Interval {
            low: Some(low),
            high: Some(high),
        }
    }

    fn reference(carrier: &Carrier, operator: BinaryOperator, left: i128, right: i128) -> i128 {
        let (left, right) = (carrier.value(left), carrier.value(right));
        value_number(
            match operator {
                BinaryOperator::BitwiseAnd => carrier.integer.bitwise_and(left, right),
                BinaryOperator::BitwiseOr => carrier.integer.bitwise_or(left, right),
                BinaryOperator::BitwiseXor => carrier.integer.bitwise_xor(left, right),
                _ => unreachable!(),
            }
            .unwrap(),
        )
    }

    #[test]
    fn exact_singletons_and_complements_cover_every_source_width() {
        for primitive in [
            PrimitiveType::I8,
            PrimitiveType::U8,
            PrimitiveType::I16,
            PrimitiveType::U16,
            PrimitiveType::I32,
            PrimitiveType::U32,
            PrimitiveType::I64,
            PrimitiveType::U64,
        ] {
            let carrier = Carrier::new(primitive).unwrap();
            for value in [
                carrier.integer.minimum_value(),
                carrier.integer.maximum_value(),
                carrier.value(0),
            ] {
                let number = value_number(value);
                let Ok(number) = i64::try_from(number) else {
                    continue;
                };
                let expected = value_number(carrier.integer.bitwise_not(value).unwrap());
                assert_eq!(
                    complement(primitive, interval(number, number), None),
                    project(expected, expected)
                );
                for operator in OPERATORS {
                    let expected = reference(&carrier, operator, number.into(), number.into());
                    assert_eq!(
                        binary(
                            operator,
                            primitive,
                            interval(number, number),
                            interval(number, number),
                            None,
                            None
                        ),
                        project(expected, expected)
                    );
                }
            }
        }
    }

    #[test]
    fn unsigned_high_constants_remain_exact_independent_of_the_sibling() {
        let primitive = PrimitiveType::U64;
        let high = Interval {
            low: Some(i64::MAX),
            high: None,
        };
        assert_eq!(
            binary(
                BinaryOperator::BitwiseAnd,
                primitive,
                interval(0, 15),
                high,
                None,
                Some(1_u64 << 63)
            ),
            interval(0, 0)
        );
        assert_eq!(
            binary(
                BinaryOperator::BitwiseAnd,
                primitive,
                interval(0, 15),
                high,
                None,
                Some(u64::MAX)
            ),
            interval(0, 15)
        );
        assert_eq!(
            binary(
                BinaryOperator::BitwiseXor,
                primitive,
                high,
                high,
                Some(u64::MAX),
                Some(u64::MAX)
            ),
            interval(0, 0)
        );
        assert_eq!(complement(primitive, high, Some(u64::MAX)), interval(0, 0));
        assert_eq!(
            binary_unsigned_value(BinaryOperator::BitwiseXor, primitive, u64::MAX, u64::MAX),
            Some(0)
        );
        assert_eq!(complement_unsigned_value(primitive, 0), Some(u64::MAX));
    }

    #[test]
    fn value_helpers_reject_wrong_carriers_and_out_of_width_values() {
        assert_eq!(complement_unsigned_value(PrimitiveType::I8, 1), None);
        assert_eq!(complement_unsigned_value(PrimitiveType::U8, 256), None);
        assert_eq!(
            binary_unsigned_value(BinaryOperator::BitwiseOr, PrimitiveType::U8, 0, 256),
            None
        );
        assert_eq!(
            binary_unsigned_value(BinaryOperator::Add, PrimitiveType::U8, 1, 2),
            None
        );
        assert_eq!(
            complement(PrimitiveType::U8, interval(0, 0), Some(256)),
            interval(0, 255)
        );
    }
}
