use std::cmp::Ordering;

use num_bigint::{BigInt, BigUint};
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};
use omega_typer::ValueType;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FloatFormat {
    Binary32,
    Binary64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FloatOperation {
    Negate,
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FloatSemantics {
    Ieee754NearestEvenV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FloatRealization {
    pub format: FloatFormat,
    pub operation: FloatOperation,
    pub semantics: FloatSemantics,
}

/// The exact IEEE representation when the computation determines one.
/// An arithmetic NaN still has semantic meaning when its bits are unspecified.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedFloat {
    Binary32(u32),
    Binary64(u64),
    UnspecifiedNaN(FloatFormat),
}

#[derive(Eq, PartialEq)]
enum Meaning {
    Finite(BigRational),
    Zero { negative: bool },
    Infinity { negative: bool },
    NaN,
}

impl FloatFormat {
    pub(super) fn for_type(carrier: &ValueType) -> Option<Self> {
        match carrier {
            ValueType::Float32 => Some(Self::Binary32),
            ValueType::Float64 => Some(Self::Binary64),
            _ => None,
        }
    }

    fn fraction_bits(self) -> i64 {
        match self {
            Self::Binary32 => 23,
            Self::Binary64 => 52,
        }
    }

    fn exponent_bits(self) -> i64 {
        match self {
            Self::Binary32 => 8,
            Self::Binary64 => 11,
        }
    }

    fn bias(self) -> i64 {
        (1 << (self.exponent_bits() - 1)) - 1
    }

    fn fraction_mask(self) -> u64 {
        (1 << self.fraction_bits()) - 1
    }

    fn sign_mask(self) -> u64 {
        1 << (self.fraction_bits() + self.exponent_bits())
    }

    fn infinity_bits(self) -> u64 {
        ((1 << self.exponent_bits()) - 1) << self.fraction_bits()
    }

    fn with_sign(
        self,
        magnitude: u64,
        negative: bool,
    ) -> PreparedFloat {
        let bits = magnitude | if negative { self.sign_mask() } else { 0 };
        match self {
            Self::Binary32 => PreparedFloat::Binary32(bits as u32),
            Self::Binary64 => PreparedFloat::Binary64(bits),
        }
    }

    pub(super) fn land(
        self,
        value: &BigRational,
    ) -> PreparedFloat {
        self.round(value, value.is_negative())
    }

    pub(super) fn land_exact_integer(
        self,
        integer: BigInt,
    ) -> Option<PreparedFloat> {
        let value = BigRational::from_integer(integer);
        let landed = self.land(&value);
        let exact = match landed.meaning() {
            Meaning::Finite(result) => result == value,
            Meaning::Zero { negative: false } => value.is_zero(),
            _ => false,
        };
        exact.then_some(landed)
    }

    fn round(
        self,
        value: &BigRational,
        negative: bool,
    ) -> PreparedFloat {
        let numerator = value.numer().magnitude();
        let denominator = value.denom().magnitude();
        if numerator.is_zero() {
            return self.with_sign(0, negative);
        }
        let minimum_exponent = 1 - self.bias();
        let minimum_scale = minimum_exponent - self.fraction_bits();
        // The bit lengths bound floor(log2(value)) to this estimate or one
        // below. Reject extremes before shifting an exact literal's large terms.
        let estimate = i128::from(numerator.bits()) - i128::from(denominator.bits());
        if estimate < i128::from(minimum_scale - 1) {
            return self.with_sign(0, negative);
        }
        if estimate > i128::from(self.bias() + 1) {
            return self.with_sign(self.infinity_bits(), negative);
        }
        let mut exponent = estimate as i64;
        let below_power = if exponent < 0 {
            &(numerator << -exponent as usize) < denominator
        } else {
            numerator < &(denominator << exponent as usize)
        };
        if below_power {
            exponent -= 1;
        }
        if exponent > self.bias() {
            return self.with_sign(self.infinity_bits(), negative);
        }
        let scale = exponent.max(minimum_exponent) - self.fraction_bits();
        let mut significand = nearest_even(numerator, denominator, scale);
        if significand == 1 << (self.fraction_bits() + 1) {
            significand >>= 1;
            exponent += 1;
        }
        if exponent > self.bias() {
            return self.with_sign(self.infinity_bits(), negative);
        }
        if significand <= self.fraction_mask() {
            return self.with_sign(significand, negative);
        }
        let exponent_bits = ((exponent.max(minimum_exponent) + self.bias()) as u64) << self.fraction_bits();
        self.with_sign(exponent_bits | (significand & self.fraction_mask()), negative)
    }

    fn realize(
        self,
        meaning: Meaning,
    ) -> PreparedFloat {
        match meaning {
            Meaning::Finite(value) => self.land(&value),
            Meaning::Zero { negative } => self.with_sign(0, negative),
            Meaning::Infinity { negative } => self.with_sign(self.infinity_bits(), negative),
            Meaning::NaN => PreparedFloat::UnspecifiedNaN(self),
        }
    }
}

fn nearest_even(
    numerator: &BigUint,
    denominator: &BigUint,
    scale: i64,
) -> u64 {
    let scaled;
    let (numerator, denominator) = if scale < 0 {
        scaled = numerator << -scale as usize;
        (&scaled, denominator)
    } else {
        scaled = denominator << scale as usize;
        (numerator, &scaled)
    };
    let quotient = (numerator / denominator)
        .to_u64()
        .expect("a binary64 significand fits in u64");
    let twice_remainder = (numerator - denominator * quotient) << 1usize;
    quotient + u64::from(twice_remainder > *denominator || twice_remainder == *denominator && quotient % 2 == 1)
}

impl PreparedFloat {
    pub fn format(self) -> FloatFormat {
        match self {
            Self::Binary32(_) => FloatFormat::Binary32,
            Self::Binary64(_) => FloatFormat::Binary64,
            Self::UnspecifiedNaN(format) => format,
        }
    }

    fn meaning(self) -> Meaning {
        let format = self.format();
        let bits = match self {
            Self::Binary32(bits) => u64::from(bits),
            Self::Binary64(bits) => bits,
            Self::UnspecifiedNaN(_) => return Meaning::NaN,
        };
        let negative = bits & format.sign_mask() != 0;
        let exponent_bits = bits & format.infinity_bits();
        let fraction = bits & format.fraction_mask();
        if exponent_bits == format.infinity_bits() {
            return if fraction == 0 { Meaning::Infinity { negative } } else { Meaning::NaN };
        }
        if exponent_bits == 0 && fraction == 0 {
            return Meaning::Zero { negative };
        }
        let (significand, exponent) = if exponent_bits == 0 {
            (fraction, 1 - format.bias())
        } else {
            (
                fraction | (1 << format.fraction_bits()),
                (exponent_bits >> format.fraction_bits()) as i64 - format.bias(),
            )
        };
        let significand = BigInt::from(significand);
        let signed = if negative { -significand } else { significand };
        let scale = exponent - format.fraction_bits();
        Meaning::Finite(if scale < 0 {
            BigRational::new(signed, BigInt::from(1) << -scale as usize)
        } else {
            BigRational::from_integer(signed << scale as usize)
        })
    }

    pub(super) fn exactly_in(
        self,
        format: FloatFormat,
    ) -> Option<Self> {
        if self.format() == format {
            return Some(self);
        }
        let meaning = self.meaning();
        let converted = match &meaning {
            Meaning::Finite(value) => format.land(value),
            Meaning::Zero { negative } => format.with_sign(0, *negative),
            Meaning::Infinity { negative } => format.with_sign(format.infinity_bits(), *negative),
            Meaning::NaN => Self::UnspecifiedNaN(format),
        };
        (converted.meaning() == meaning).then_some(converted)
    }

    pub(super) fn exact_integer(self) -> Option<BigInt> {
        match self.meaning() {
            Meaning::Finite(value) if value.is_integer() => Some(value.to_integer()),
            Meaning::Zero { negative: false } => Some(BigInt::ZERO),
            _ => None,
        }
    }
}

impl FloatSemantics {
    pub(super) fn negate(
        self,
        value: PreparedFloat,
    ) -> PreparedFloat {
        match self {
            Self::Ieee754NearestEvenV1 => value.format().realize(value.meaning().negated()),
        }
    }

    pub(super) fn arithmetic(
        self,
        operation: FloatOperation,
        left: PreparedFloat,
        right: PreparedFloat,
    ) -> PreparedFloat {
        use FloatOperation::*;
        let left_meaning = left.meaning();
        let right = right.meaning();
        let result = match (self, operation) {
            (Self::Ieee754NearestEvenV1, Add) => left_meaning.add(right),
            (Self::Ieee754NearestEvenV1, Subtract) => left_meaning.add(right.negated()),
            (Self::Ieee754NearestEvenV1, Multiply) => left_meaning.multiply(right),
            (Self::Ieee754NearestEvenV1, Divide) => left_meaning.divide(right),
            _ => unreachable!("selected a binary arithmetic operation"),
        };
        left.format().realize(result)
    }

    pub(super) fn compare(
        self,
        operation: FloatOperation,
        left: PreparedFloat,
        right: PreparedFloat,
    ) -> bool {
        use FloatOperation::*;
        let ordering = match self {
            Self::Ieee754NearestEvenV1 => left.meaning().compare(right.meaning()),
        };
        match operation {
            Equal => ordering == Some(Ordering::Equal),
            NotEqual => ordering != Some(Ordering::Equal),
            Less => ordering == Some(Ordering::Less),
            LessEqual => matches!(ordering, Some(Ordering::Less | Ordering::Equal)),
            Greater => ordering == Some(Ordering::Greater),
            GreaterEqual => matches!(ordering, Some(Ordering::Greater | Ordering::Equal)),
            _ => unreachable!("selected an IEEE comparison"),
        }
    }
}

impl Meaning {
    fn negative(&self) -> bool {
        match self {
            Self::Finite(value) => value.is_negative(),
            Self::Zero { negative } | Self::Infinity { negative } => *negative,
            Self::NaN => false,
        }
    }

    fn negated(self) -> Self {
        match self {
            Self::Finite(value) => Self::Finite(-value),
            Self::Zero { negative } => Self::Zero { negative: !negative },
            Self::Infinity { negative } => Self::Infinity { negative: !negative },
            Self::NaN => Self::NaN,
        }
    }

    fn add(
        self,
        right: Self,
    ) -> Self {
        use Meaning::*;
        match (self, right) {
            (NaN, _) | (_, NaN) => NaN,
            (Infinity { negative: a }, Infinity { negative: b }) if a != b => NaN,
            (value @ Infinity { .. }, _) | (_, value @ Infinity { .. }) => value,
            (Zero { negative: a }, Zero { negative: b }) => Zero { negative: a && b },
            (value, Zero { .. }) | (Zero { .. }, value) => value,
            (Finite(a), Finite(b)) => Finite(a + b),
        }
    }

    fn multiply(
        self,
        right: Self,
    ) -> Self {
        use Meaning::*;
        let negative = self.negative() ^ right.negative();
        match (self, right) {
            (NaN, _) | (_, NaN) | (Zero { .. }, Infinity { .. }) | (Infinity { .. }, Zero { .. }) => NaN,
            (Infinity { .. }, _) | (_, Infinity { .. }) => Infinity { negative },
            (Zero { .. }, _) | (_, Zero { .. }) => Zero { negative },
            (Finite(a), Finite(b)) => Finite(a * b),
        }
    }

    fn divide(
        self,
        right: Self,
    ) -> Self {
        use Meaning::*;
        let negative = self.negative() ^ right.negative();
        match (self, right) {
            (NaN, _) | (_, NaN) | (Zero { .. }, Zero { .. }) | (Infinity { .. }, Infinity { .. }) => NaN,
            (Infinity { .. }, _) | (_, Zero { .. }) => Infinity { negative },
            (Zero { .. }, _) | (_, Infinity { .. }) => Zero { negative },
            (Finite(a), Finite(b)) => Finite(a / b),
        }
    }

    fn compare(
        self,
        right: Self,
    ) -> Option<Ordering> {
        use Meaning::*;
        Some(match (self, right) {
            (NaN, _) | (_, NaN) => return None,
            (Infinity { negative: a }, Infinity { negative: b }) => b.cmp(&a),
            (_, Infinity { negative: false }) | (Infinity { negative: true }, _) => Ordering::Less,
            (_, Infinity { negative: true }) | (Infinity { negative: false }, _) => Ordering::Greater,
            (Zero { .. }, Zero { .. }) => Ordering::Equal,
            (Zero { .. }, Finite(b)) => BigRational::zero().cmp(&b),
            (Finite(a), Zero { .. }) => a.cmp(&BigRational::zero()),
            (Finite(a), Finite(b)) => a.cmp(&b),
        })
    }
}
