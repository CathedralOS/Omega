//! Numeric hulls of fixed-width bitwise operations on admitted intervals.
//! Endpoint operations alone are unsound: each interval supplies a common
//! bit prefix, and the operation transfers those fixed bits before projecting
//! their signed or unsigned hull. Source and selected-plan checking share this
//! arithmetic owner; neither needs its own bit-pattern approximation.

use super::{IntegerSign, IntegerType, IntegerValue, Ordering};

#[derive(Clone, Copy)]
struct KnownBits {
    ones: u128,
    zeros: u128,
}

#[cfg(test)]
mod tests {
    use super::{IntegerSign, IntegerType, IntegerValue};

    #[test]
    fn bitwise_bounds_cover_every_small_signed_and_unsigned_interval() {
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            let integer = IntegerType::new(sign, 4).unwrap();
            let (minimum, maximum) = match sign {
                IntegerSign::Signed => (-8, 7),
                IntegerSign::Unsigned => (0, 15),
            };
            let value = |number| match sign {
                IntegerSign::Signed => IntegerValue::Signed(number),
                IntegerSign::Unsigned => IntegerValue::Unsigned(number as u128),
            };
            for left_low in minimum..=maximum {
                for left_high in left_low..=maximum {
                    for right_low in minimum..=maximum {
                        for right_high in right_low..=maximum {
                            let left = (value(left_low), value(left_high));
                            let right = (value(right_low), value(right_high));
                            let bounds = [
                                integer.bitwise_and_bounds(left, right).unwrap(),
                                integer.bitwise_or_bounds(left, right).unwrap(),
                                integer.bitwise_xor_bounds(left, right).unwrap(),
                            ];
                            for left in left_low..=left_high {
                                for right in right_low..=right_high {
                                    let results = [
                                        integer.bitwise_and(value(left), value(right)).unwrap(),
                                        integer.bitwise_or(value(left), value(right)).unwrap(),
                                        integer.bitwise_xor(value(left), value(right)).unwrap(),
                                    ];
                                    for ((low, high), result) in bounds.iter().zip(results) {
                                        assert!(!integer.compare(*low, result).unwrap().is_gt());
                                        assert!(!integer.compare(result, *high).unwrap().is_gt());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn byte_mask_retains_ascii_prefix_and_full_width_constants() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let value = IntegerValue::Unsigned;
        assert_eq!(
            integer.bitwise_and_bounds((value(48), value(57)), (value(255), value(255))),
            Some((value(48), value(63)))
        );
        let integer = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            integer.bitwise_and_bounds((value(0), value(u128::MAX)), (value(255), value(255))),
            Some((value(0), value(255)))
        );
    }
}

impl IntegerType {
    pub fn bitwise_and_bounds(
        self,
        left: (IntegerValue, IntegerValue),
        right: (IntegerValue, IntegerValue),
    ) -> Option<(IntegerValue, IntegerValue)> {
        self.bitwise_bounds(left, right, |left, right| KnownBits {
            ones: left.ones & right.ones,
            zeros: left.zeros | right.zeros,
        })
    }

    pub fn bitwise_or_bounds(
        self,
        left: (IntegerValue, IntegerValue),
        right: (IntegerValue, IntegerValue),
    ) -> Option<(IntegerValue, IntegerValue)> {
        self.bitwise_bounds(left, right, |left, right| KnownBits {
            ones: left.ones | right.ones,
            zeros: left.zeros & right.zeros,
        })
    }

    pub fn bitwise_xor_bounds(
        self,
        left: (IntegerValue, IntegerValue),
        right: (IntegerValue, IntegerValue),
    ) -> Option<(IntegerValue, IntegerValue)> {
        self.bitwise_bounds(left, right, |left, right| KnownBits {
            ones: (left.ones & right.zeros) | (left.zeros & right.ones),
            zeros: (left.ones & right.ones) | (left.zeros & right.zeros),
        })
    }

    fn bitwise_bounds(
        self,
        left: (IntegerValue, IntegerValue),
        right: (IntegerValue, IntegerValue),
        transfer: impl FnOnce(KnownBits, KnownBits) -> KnownBits,
    ) -> Option<(IntegerValue, IntegerValue)> {
        let bits = transfer(
            self.interval_known_bits(left)?,
            self.interval_known_bits(right)?,
        );
        let sign_bit = 1_u128 << (self.bits - 1);
        let mut low = bits.ones;
        let mut high = self.bit_range_mask() & !bits.zeros;
        if self.sign == IntegerSign::Signed && (bits.ones | bits.zeros) & sign_bit == 0 {
            low |= sign_bit;
            high &= !sign_bit;
        }
        Some((self.bit_range_value(low), self.bit_range_value(high)))
    }

    fn interval_known_bits(self, (low, high): (IntegerValue, IntegerValue)) -> Option<KnownBits> {
        if !self.admits(low) || !self.admits(high) || self.compare(low, high)? == Ordering::Greater
        {
            return None;
        }
        let bits = |value| match value {
            IntegerValue::Signed(value) => value as u128,
            IntegerValue::Unsigned(value) => value,
        };
        if matches!((low, high), (IntegerValue::Signed(low), IntegerValue::Signed(high)) if low < 0 && high >= 0)
        {
            return Some(KnownBits { ones: 0, zeros: 0 });
        }
        let mask = self.bit_range_mask();
        let low = bits(low) & mask;
        let difference = low ^ (bits(high) & mask);
        let variable_width = u128::BITS - difference.leading_zeros();
        let variable = match variable_width {
            0 => 0,
            128 => u128::MAX,
            width => (1_u128 << width) - 1,
        };
        let fixed = mask & !variable;
        Some(KnownBits {
            ones: low & fixed,
            zeros: !low & fixed,
        })
    }

    fn bit_range_mask(self) -> u128 {
        if self.bits == 128 {
            u128::MAX
        } else {
            (1_u128 << self.bits) - 1
        }
    }

    fn bit_range_value(self, bits: u128) -> IntegerValue {
        match self.sign {
            IntegerSign::Unsigned => IntegerValue::Unsigned(bits),
            IntegerSign::Signed => {
                let mask = self.bit_range_mask();
                let extended = if bits & (1_u128 << (self.bits - 1)) != 0 {
                    bits | !mask
                } else {
                    bits
                };
                IntegerValue::Signed(extended as i128)
            }
        }
    }
}
