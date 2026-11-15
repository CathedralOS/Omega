//! Independent arithmetic for complete miniature IEEE formats. Finite values
//! are integer multiples of 2^-12. Conversion enumerates every destination
//! magnitude and selects minimum distance, with even-significand tie breaking.
//! This module uses neither production float semantics nor host float arithmetic.

#[derive(Debug, Clone, Copy)]
pub(super) struct Format {
    pub precision: u32,
    pub exponent_bits: u32,
    pub bias: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Meaning {
    Finite(i64),
    Zero(bool),
    Infinity(bool),
    NaN,
}

pub(super) const NARROW: Format = Format {
    precision: 3,
    exponent_bits: 2,
    bias: 1,
};
pub(super) const WIDE: Format = Format {
    precision: 4,
    exponent_bits: 3,
    bias: 3,
};

impl Format {
    fn fraction_bits(self) -> u32 {
        self.precision - 1
    }

    fn infinity(self) -> u32 {
        ((1 << self.exponent_bits) - 1) << self.fraction_bits()
    }

    fn sign(self) -> u32 {
        1 << (self.exponent_bits + self.fraction_bits())
    }

    fn maximum_exponent(self) -> i32 {
        (1_i32 << self.exponent_bits) - 2 - self.bias
    }

    fn minimum_quantum(self) -> i32 {
        1 - self.bias - self.fraction_bits() as i32
    }

    pub fn encodings(self) -> std::ops::Range<u32> {
        0..2 * self.sign()
    }

    pub fn decode(self, bits: u32) -> Meaning {
        let negative = bits & self.sign() != 0;
        let exponent = (bits >> self.fraction_bits()) & ((1 << self.exponent_bits) - 1);
        let fraction = bits & ((1 << self.fraction_bits()) - 1);
        if exponent == (1 << self.exponent_bits) - 1 {
            return if fraction == 0 {
                Meaning::Infinity(negative)
            } else {
                Meaning::NaN
            };
        }
        if exponent == 0 && fraction == 0 {
            return Meaning::Zero(negative);
        }
        let significand = if exponent == 0 {
            fraction
        } else {
            (1 << self.fraction_bits()) + fraction
        };
        let exponent = if exponent == 0 {
            1 - self.bias
        } else {
            exponent as i32 - self.bias
        };
        let shift = exponent - self.fraction_bits() as i32 + 12;
        assert!(
            shift >= 0,
            "test-local fixed quantum must represent every model value"
        );
        let magnitude = i64::from(significand) << shift;
        Meaning::Finite(if negative { -magnitude } else { magnitude })
    }

    pub fn round(self, meaning: Meaning) -> u32 {
        let (negative, magnitude) = match meaning {
            Meaning::NaN => return self.infinity() | (1 << (self.fraction_bits() - 1)),
            Meaning::Infinity(negative) => {
                return self.infinity() | if negative { self.sign() } else { 0 };
            }
            Meaning::Zero(negative) => return if negative { self.sign() } else { 0 },
            Meaning::Finite(value) => (value < 0, value.abs()),
        };
        let sign = if negative { self.sign() } else { 0 };
        let Meaning::Finite(largest) = self.decode(self.infinity() - 1) else {
            unreachable!()
        };
        let overflow_midpoint = (largest + (1_i64 << (self.maximum_exponent() + 1 + 12))) / 2;
        if magnitude >= overflow_midpoint {
            return sign | self.infinity();
        }
        let nearest = (0..self.infinity())
            .min_by_key(|&bits| {
                let value = match self.decode(bits) {
                    Meaning::Zero(_) => 0,
                    Meaning::Finite(value) => value,
                    _ => unreachable!(),
                };
                ((value - magnitude).abs(), bits & 1)
            })
            .unwrap();
        sign | nearest
    }

    pub fn embeds_into(self, target: Self) -> bool {
        target.precision >= self.precision
            && target.maximum_exponent() >= self.maximum_exponent()
            && target.minimum_quantum() <= self.minimum_quantum()
    }

    pub fn roundtrip(self, target: Self, bits: u32) -> Meaning {
        self.decode(self.round(target.decode(target.round(self.decode(bits)))))
    }

    /// Exact embedding into a permanent binary format for certificate tests.
    /// Only integer bit construction is used; NaN sign, quiet bit and payload
    /// are retained here even though Meaning equality erases their distinction.
    pub fn binary_bits(self, bits: u32, fraction_bits: u32, exponent_bits: u32, bias: i32) -> u64 {
        let negative = bits & self.sign() != 0;
        let sign = if negative {
            1_u64 << (fraction_bits + exponent_bits)
        } else {
            0
        };
        let infinity = ((1_u64 << exponent_bits) - 1) << fraction_bits;
        match self.decode(bits) {
            Meaning::Zero(_) => sign,
            Meaning::Infinity(_) => sign | infinity,
            Meaning::NaN => {
                let fraction = bits & ((1 << self.fraction_bits()) - 1);
                sign | infinity | (u64::from(fraction) << (fraction_bits - self.fraction_bits()))
            }
            Meaning::Finite(value) => {
                let magnitude = value.unsigned_abs();
                let leading = 63 - magnitude.leading_zeros();
                let exponent = leading as i32 - 12;
                let significand = magnitude << (fraction_bits - leading);
                sign | (((exponent + bias) as u64) << fraction_bits)
                    | (significand & ((1_u64 << fraction_bits) - 1))
            }
        }
    }
}
