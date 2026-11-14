/*
We represent canonical integer and floating-point source literals (`IntegerLiteral`,
`FloatLiteral`) with their landed carrier metadata (`IntegerLanding`,
`LandedIntegerType`, `FloatFormat`). Small integer magnitudes stay inline as
`u64` while arbitrary-width literals retain canonical text, and float literals
preserve their exact decimal spelling so every format read rounds once through
`FloatSemantics` without double-rounding.
*/

use crate::integer_carrier::FixedInteger;
use std::borrow::Cow;
use std::sync::Arc;

/*
    The carrier an integer literal lands in: a fixed-width integer of any
    sign and width, or the pointer-width address, which holds the unsigned
    64-bit range but is not an integer count.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum LandedIntegerType {
    Fixed(FixedInteger),
    Addr,
}

impl LandedIntegerType {
    pub const U8: Self = Self::Fixed(FixedInteger::U8);
    pub const U64: Self = Self::Fixed(FixedInteger::U64);

    pub fn from_name(name: &str) -> Option<Self> {
        if name == "addr" {
            return Some(Self::Addr);
        }
        FixedInteger::from_decimal_spelling(name).map(Self::Fixed)
    }

    /* The integer whose range and word operations the landed value has. */
    pub fn value_carrier(self) -> FixedInteger {
        match self {
            Self::Fixed(carrier) => carrier,
            Self::Addr => FixedInteger::U64,
        }
    }

    /* `None` when an endpoint exceeds a host integer; see `FixedInteger::bounds_i128`. */
    pub fn bounds(self) -> Option<(i128, i128)> {
        self.value_carrier().bounds_i128()
    }

    /* Membership of a host integer in the landed range, exact at every width. */
    pub fn admits_i128(self, value: i128) -> bool {
        self.value_carrier()
            .admits(&crate::bignum::BigInt::from_i128(value))
    }

    pub fn bit_width(self) -> u64 {
        self.value_carrier().width()
    }

    pub fn is_signed(self) -> bool {
        matches!(self, Self::Fixed(carrier) if carrier.is_signed())
    }

    pub fn name(self) -> String {
        self.to_string()
    }
}

impl std::fmt::Display for LandedIntegerType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fixed(carrier) => carrier.fmt(formatter),
            Self::Addr => formatter.write_str("addr"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct IntegerLanding {
    pub landed_type: LandedIntegerType,
    pub domain: crate::arithmetic::ArithmeticDomain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum FloatFormat {
    F32,
    F64,
}

impl FloatFormat {
    pub fn name(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }

    pub fn widen_representation(self, bits: u64) -> Option<f64> {
        match self {
            Self::F32 => Some(crate::float_representation::binary32_from_bits(
                u32::try_from(bits).ok()?,
            )),
            Self::F64 => Some(f64::from_bits(bits)),
        }
    }

    pub fn capture_wide_representation(self, value: f64) -> Option<u64> {
        match self {
            Self::F32 => {
                let bits = crate::float_representation::binary32_to_bits(value);
                (crate::float_representation::binary32_from_bits(bits).to_bits() == value.to_bits())
                    .then_some(u64::from(bits))
            }
            Self::F64 => Some(value.to_bits()),
        }
    }
}

#[derive(Debug, Clone, compiler_storage::Record)]
pub struct FloatLiteral {
    text: Arc<str>,
    landing: Option<FloatFormat>,
    presented: Option<(FloatFormat, u64)>,
}

impl PartialEq for FloatLiteral {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text && self.presented == other.presented
    }
}

impl Eq for FloatLiteral {}

impl Default for FloatLiteral {
    fn default() -> Self {
        Self::from_f64(0.0)
    }
}

impl FloatLiteral {
    pub fn new(value: f64) -> Self {
        Self::from_f64(value)
    }

    pub fn parse(source: &str) -> Option<Self> {
        let (body, landing) = strip_float_literal_suffix(source);
        let body = if body.contains('_') {
            Cow::Owned(body.replace('_', ""))
        } else {
            Cow::Borrowed(body)
        };
        crate::bignum::ExactFloat::from_decimal_str(&body)?;
        Some(Self {
            text: Arc::from(body.as_ref()),
            landing,
            presented: None,
        })
    }

    pub fn from_f64(value: f64) -> Self {
        Self {
            text: Arc::from(format!("{value:?}")),
            landing: None,
            presented: None,
        }
    }

    pub fn from_bits(format: FloatFormat, bits: u64) -> Option<Self> {
        let value = format.widen_representation(bits)?;
        let text = match format {
            FloatFormat::F32 => format!("{:?}", f32::from_bits(bits as u32)),
            FloatFormat::F64 => format!("{value:?}"),
        };
        Some(Self {
            text: Arc::from(text),
            landing: Some(format),
            presented: Some((format, bits)),
        })
    }

    pub fn presented_bits(&self) -> Option<(FloatFormat, u64)> {
        self.presented
    }

    pub fn value_f64(&self) -> f64 {
        if let Some((format, bits)) = self.presented {
            return format
                .widen_representation(bits)
                .expect("validated floating representation");
        }
        crate::float_semantics::FloatSemantics::from_decimal(
            crate::float_semantics::FloatFormat::BINARY64,
            &self.text,
        )
        .map(|meaning| meaning.to_f64())
        .unwrap_or(0.0)
    }

    pub fn value_f32(&self) -> f32 {
        if let Some((format, bits)) = self.presented {
            return match format {
                FloatFormat::F32 => f32::from_bits(bits as u32),
                FloatFormat::F64 => f64::from_bits(bits) as f32,
            };
        }
        crate::float_semantics::FloatSemantics::from_decimal(
            crate::float_semantics::FloatFormat::BINARY32,
            &self.text,
        )
        .map(|meaning| meaning.to_f32())
        .unwrap_or(0.0)
    }

    pub fn value(&self) -> f64 {
        self.value_f64()
    }

    pub fn f32_bits(&self) -> u32 {
        self.value_f32().to_bits()
    }

    pub fn landed_f64(&self) -> f64 {
        if let Some((format, bits)) = self.presented {
            return format
                .widen_representation(bits)
                .expect("validated floating representation");
        }
        match self.landing {
            Some(FloatFormat::F32) => f64::from(self.value_f32()),
            _ => self.value_f64(),
        }
    }

    pub fn with_landing(&self, landing: FloatFormat) -> Self {
        Self {
            text: self.text.clone(),
            landing: Some(self.presented.map_or(landing, |(format, _)| format)),
            presented: self.presented,
        }
    }

    pub fn landing(&self) -> Option<FloatFormat> {
        self.landing
    }

    pub fn negated(&self) -> Self {
        if let Some((format, bits)) = self.presented {
            let sign = match format {
                FloatFormat::F32 => 1u64 << 31,
                FloatFormat::F64 => 1u64 << 63,
            };
            return Self::from_bits(format, bits ^ sign)
                .expect("validated floating representation");
        }
        let text: &str = &self.text;
        let flipped = match text.strip_prefix('-') {
            Some(positive) => Arc::from(positive),
            None => Arc::from(format!("-{text}")),
        };
        Self {
            text: flipped,
            landing: self.landing,
            presented: None,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

impl std::fmt::Display for FloatLiteral {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.text)
    }
}

fn strip_float_literal_suffix(source: &str) -> (&str, Option<FloatFormat>) {
    if let Some(body) = source.strip_suffix("f32") {
        return (body, Some(FloatFormat::F32));
    }
    if let Some(body) = source.strip_suffix("f64") {
        return (body, Some(FloatFormat::F64));
    }
    (source.trim_end_matches(['f', 'F']), None)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum IntegerRadix {
    Binary,
    Octal,
    Decimal,
    Hexadecimal,
}

impl IntegerRadix {
    fn base(self) -> u32 {
        match self {
            Self::Binary => 2,
            Self::Octal => 8,
            Self::Decimal => 10,
            Self::Hexadecimal => 16,
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::Binary => "0b",
            Self::Octal => "0o",
            Self::Decimal => "",
            Self::Hexadecimal => "0x",
        }
    }
}

#[derive(Clone, compiler_storage::Record)]
pub struct IntegerLiteral {
    representation: IntegerRepresentation,
}

#[derive(Clone, compiler_storage::Record)]
enum IntegerRepresentation {
    Small {
        magnitude: u64,
        radix: IntegerRadix,
        negative: bool,
        landing: Option<IntegerLanding>,
    },
    Large {
        text: Arc<String>,
        landing: Option<IntegerLanding>,
    },
}

pub struct CanonicalIntegerSpelling<'literal> {
    storage: IntegerSpellingStorage<'literal>,
}

enum IntegerSpellingStorage<'literal> {
    Borrowed(&'literal str),
    Inline { bytes: [u8; 67], start: u8 },
}

impl CanonicalIntegerSpelling<'_> {
    pub fn as_str(&self) -> &str {
        match &self.storage {
            IntegerSpellingStorage::Borrowed(text) => text,
            IntegerSpellingStorage::Inline { bytes, start } => {
                std::str::from_utf8(&bytes[usize::from(*start)..])
                    .expect("canonical integer formatting writes only ASCII")
            }
        }
    }
}

impl std::fmt::Debug for CanonicalIntegerSpelling<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl std::fmt::Display for CanonicalIntegerSpelling<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::fmt::Debug for IntegerLiteral {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IntegerLiteral")
            .field("text", &self.canonical_spelling())
            .field("landing", &self.landing())
            .finish()
    }
}

impl PartialEq for IntegerLiteral {
    fn eq(&self, other: &Self) -> bool {
        match (&self.representation, &other.representation) {
            (
                IntegerRepresentation::Small {
                    magnitude: left,
                    radix: left_radix,
                    negative: left_negative,
                    ..
                },
                IntegerRepresentation::Small {
                    magnitude: right,
                    radix: right_radix,
                    negative: right_negative,
                    ..
                },
            ) => left == right && left_radix == right_radix && left_negative == right_negative,
            (
                IntegerRepresentation::Large { text: left, .. },
                IntegerRepresentation::Large { text: right, .. },
            ) => left == right,
            _ => false,
        }
    }
}

impl Eq for IntegerLiteral {}

impl IntegerLiteral {
    pub fn from_parts(
        negative: bool,
        radix: IntegerRadix,
        digits: &str,
    ) -> Result<Self, &'static str> {
        let mut magnitude = Some(0u64);
        let mut saw_digit = false;
        for character in digits.chars() {
            if character == '_' {
                continue;
            }
            let digit = character
                .to_digit(radix.base())
                .ok_or("invalid integer literal")?;
            saw_digit = true;
            magnitude = magnitude.and_then(|value| {
                value
                    .checked_mul(u64::from(radix.base()))?
                    .checked_add(u64::from(digit))
            });
        }
        if !saw_digit {
            return Err("invalid integer literal");
        }
        if let Some(magnitude) = magnitude {
            return Ok(Self {
                representation: IntegerRepresentation::Small {
                    magnitude,
                    radix,
                    negative: negative && magnitude != 0,
                    landing: None,
                },
            });
        }
        let canonical_digits = digits
            .bytes()
            .filter(|byte| *byte != b'_')
            .skip_while(|byte| *byte == b'0')
            .count();
        let mut text =
            String::with_capacity(canonical_digits + radix.prefix().len() + usize::from(negative));
        if negative {
            text.push('-');
        }
        text.push_str(radix.prefix());
        let mut significant = false;
        for character in digits.chars().filter(|character| *character != '_') {
            significant |= character != '0';
            if significant {
                text.push(character.to_ascii_lowercase());
            }
        }
        Ok(Self {
            representation: IntegerRepresentation::Large {
                text: Arc::new(text),
                landing: None,
            },
        })
    }

    pub fn from_value(value: i64) -> Self {
        Self {
            representation: IntegerRepresentation::Small {
                magnitude: value.unsigned_abs(),
                radix: IntegerRadix::Decimal,
                negative: value < 0,
                landing: None,
            },
        }
    }

    pub fn zero() -> Self {
        Self::from_value(0)
    }

    pub fn with_landing(&self, landing: IntegerLanding) -> Self {
        let mut result = self.clone();
        match &mut result.representation {
            IntegerRepresentation::Small { landing: slot, .. }
            | IntegerRepresentation::Large { landing: slot, .. } => *slot = Some(landing),
        }
        result
    }

    pub fn landing(&self) -> Option<IntegerLanding> {
        match self.representation {
            IntegerRepresentation::Small { landing, .. }
            | IntegerRepresentation::Large { landing, .. } => landing,
        }
    }

    pub fn is_negative(&self) -> bool {
        match &self.representation {
            IntegerRepresentation::Small { negative, .. } => *negative,
            IntegerRepresentation::Large { text, .. } => text.starts_with('-'),
        }
    }

    pub fn negated(&self) -> Self {
        match &self.representation {
            IntegerRepresentation::Small {
                magnitude,
                radix,
                negative,
                landing,
            } => Self {
                representation: IntegerRepresentation::Small {
                    magnitude: *magnitude,
                    radix: *radix,
                    landing: *landing,
                    negative: !(*magnitude == 0 && *radix == IntegerRadix::Decimal) && !negative,
                },
            },
            IntegerRepresentation::Large { text, landing } => Self {
                representation: IntegerRepresentation::Large {
                    text: Arc::new(match text.strip_prefix('-') {
                        Some(positive) => positive.to_owned(),
                        None => format!("-{text}"),
                    }),
                    landing: *landing,
                },
            },
        }
    }

    pub fn value_i64(&self) -> Option<i64> {
        let IntegerRepresentation::Small {
            magnitude,
            negative,
            ..
        } = self.representation
        else {
            return None;
        };
        if negative {
            i64::try_from(-i128::from(magnitude)).ok()
        } else {
            i64::try_from(magnitude).ok()
        }
    }

    pub fn value_u64(&self) -> Option<u64> {
        match self.representation {
            IntegerRepresentation::Small {
                magnitude,
                negative: false,
                ..
            } => Some(magnitude),
            _ => None,
        }
    }

    pub fn value_bignum(&self) -> Option<crate::bignum::BigInt> {
        match &self.representation {
            IntegerRepresentation::Small {
                magnitude,
                negative,
                ..
            } => {
                let magnitude = crate::bignum::BigInt::from_u64(*magnitude);
                Some(if *negative {
                    magnitude.negate()
                } else {
                    magnitude
                })
            }
            IntegerRepresentation::Large { text, .. } => {
                let (negative, unsigned) = text
                    .strip_prefix('-')
                    .map_or((false, text.as_str()), |digits| (true, digits));
                let (base, digits) = split_radix(unsigned);
                let magnitude = crate::bignum::BigInt::from_str_radix(digits, base)?;
                Some(if negative {
                    magnitude.negate()
                } else {
                    magnitude
                })
            }
        }
    }

    pub fn bits_u64(&self) -> Option<u64> {
        self.value_i64()
            .map(|value| value as u64)
            .or_else(|| self.value_u64())
    }

    pub fn decimal_value_i128(&self) -> Option<i128> {
        match &self.representation {
            IntegerRepresentation::Small {
                magnitude,
                radix: IntegerRadix::Decimal,
                negative,
                ..
            } => Some(if *negative {
                -i128::from(*magnitude)
            } else {
                i128::from(*magnitude)
            }),
            IntegerRepresentation::Small { .. } => None,
            IntegerRepresentation::Large { text, .. } => text.parse().ok(),
        }
    }

    pub fn decimal_value_bignum(&self) -> Option<crate::bignum::BigInt> {
        match &self.representation {
            IntegerRepresentation::Small {
                radix: IntegerRadix::Decimal,
                ..
            } => self.value_bignum(),
            IntegerRepresentation::Small { .. } => None,
            IntegerRepresentation::Large { text, .. } => {
                crate::bignum::BigInt::from_decimal_str(text)
            }
        }
    }

    pub fn canonical_spelling(&self) -> CanonicalIntegerSpelling<'_> {
        let storage = match &self.representation {
            IntegerRepresentation::Large { text, .. } => IntegerSpellingStorage::Borrowed(text),
            IntegerRepresentation::Small {
                magnitude,
                radix,
                negative,
                ..
            } => {
                let mut bytes = [0; 67];
                let mut start = bytes.len();
                let mut remaining = *magnitude;
                let base = u64::from(radix.base());
                loop {
                    start -= 1;
                    bytes[start] = b"0123456789abcdef"[(remaining % base) as usize];
                    remaining /= base;
                    if remaining == 0 {
                        break;
                    }
                }
                let prefix = radix.prefix().as_bytes();
                start -= prefix.len();
                bytes[start..start + prefix.len()].copy_from_slice(prefix);
                if *negative {
                    start -= 1;
                    bytes[start] = b'-';
                }
                IntegerSpellingStorage::Inline {
                    bytes,
                    start: start as u8,
                }
            }
        };
        CanonicalIntegerSpelling { storage }
    }
}

impl Default for IntegerLiteral {
    fn default() -> Self {
        Self::zero()
    }
}

impl std::fmt::Display for IntegerLiteral {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.canonical_spelling().as_str())
    }
}

fn split_radix(unsigned: &str) -> (u32, &str) {
    if let Some(digits) = unsigned.strip_prefix("0b") {
        (2, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0o") {
        (8, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0x") {
        (16, digits)
    } else {
        (10, unsigned)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FixedInteger, FloatFormat, FloatLiteral, IntegerLanding, IntegerLiteral, IntegerRadix,
        LandedIntegerType,
    };

    #[test]
    fn presented_float_bits_survive_wide_storage_and_negation() {
        for (format, bits, wide) in [
            (FloatFormat::F32, 0xff80_0123, 0xfff0_0024_6000_0000),
            (FloatFormat::F32, 0x8000_0000, 0x8000_0000_0000_0000),
            (FloatFormat::F32, 0x0000_0001, 0x36a0_0000_0000_0000),
            (
                FloatFormat::F64,
                0xfff0_0000_0000_0123,
                0xfff0_0000_0000_0123,
            ),
        ] {
            let literal = FloatLiteral::from_bits(format, bits).unwrap();
            assert_eq!(literal.landed_f64().to_bits(), wide);
            assert_eq!(
                format.capture_wide_representation(f64::from_bits(wide)),
                Some(bits)
            );
            assert_eq!(literal.negated().negated(), literal);
            assert_eq!(literal.presented_bits(), Some((format, bits)));
        }
        assert!(FloatLiteral::from_bits(FloatFormat::F32, 0x1_0000_0000).is_none());
        assert_eq!(
            FloatLiteral::from_bits(FloatFormat::F32, 0xff80_0123)
                .unwrap()
                .negated(),
            FloatLiteral::from_bits(FloatFormat::F32, 0x7f80_0123).unwrap(),
        );
        assert!(
            FloatFormat::F32
                .capture_wide_representation(f64::from_bits(0x7ff0_0000_0000_0001))
                .is_none()
        );
        assert_ne!(
            FloatLiteral::from_bits(FloatFormat::F32, 0x7f80_0001),
            FloatLiteral::from_bits(FloatFormat::F32, 0x7f80_0002)
        );
        assert!(
            FloatLiteral::parse("NaNf32")
                .unwrap()
                .presented_bits()
                .is_none()
        );
    }

    #[test]
    fn canonicalization_is_spelling_insensitive_within_a_radix() {
        let plain = IntegerLiteral::from_parts(false, IntegerRadix::Hexadecimal, "FF").unwrap();
        let fancy = IntegerLiteral::from_parts(false, IntegerRadix::Hexadecimal, "0_0fF").unwrap();
        assert_eq!(plain, fancy);
        assert_eq!(plain.canonical_spelling().as_str(), "0xff");
    }

    #[test]
    fn value_i64_matches_the_old_parser_for_in_range_literals() {
        let literal = IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "42").unwrap();
        assert_eq!(literal.value_i64(), Some(42));
        assert_eq!(literal.negated().value_i64(), Some(-42));
        assert_eq!(IntegerLiteral::from_value(-7).value_i64(), Some(-7));
    }

    #[test]
    fn u64_magnitudes_parse_but_do_not_fit_i64() {
        let max = IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "18446744073709551615")
            .unwrap();
        assert_eq!(max.canonical_spelling().as_str(), "18446744073709551615");
        assert_eq!(max.value_i64(), None);
        assert_eq!(max.value_u64(), Some(u64::MAX));
        assert_eq!(max.bits_u64(), Some(u64::MAX));
    }

    #[test]
    fn u64_window_rejects_negatives_and_bits_are_twos_complement() {
        let negative = IntegerLiteral::from_value(-1);
        assert_eq!(negative.value_u64(), None);
        assert_eq!(negative.bits_u64(), Some(u64::MAX));
        let beyond =
            IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "18446744073709551616")
                .unwrap();
        assert_eq!(beyond.value_u64(), None);
        assert_eq!(beyond.bits_u64(), None);
    }

    #[test]
    fn i64_min_is_directly_spellable_via_negation() {
        let magnitude =
            IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "9223372036854775808")
                .unwrap();
        assert_eq!(magnitude.value_i64(), None);
        assert_eq!(magnitude.negated().value_i64(), Some(i64::MIN));
    }

    #[test]
    fn zero_never_carries_a_sign_and_double_negation_round_trips() {
        let zero = IntegerLiteral::from_parts(true, IntegerRadix::Decimal, "000").unwrap();
        assert_eq!(zero.canonical_spelling().as_str(), "0");
        assert_eq!(zero.negated().canonical_spelling().as_str(), "0");
        let five = IntegerLiteral::from_value(5);
        assert_eq!(five.negated().negated(), five);
    }

    #[test]
    fn invalid_digits_reject() {
        assert!(IntegerLiteral::from_parts(false, IntegerRadix::Binary, "102").is_err());
        assert!(IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "___").is_err());
    }

    #[test]
    fn landing_rides_clones_and_negation_but_not_equality() {
        let landing = IntegerLanding {
            landed_type: LandedIntegerType::U64,
            domain: crate::arithmetic::ArithmeticDomain::Wrapping,
        };
        let anonymous = IntegerLiteral::from_value(5);
        let landed = anonymous.with_landing(landing);
        assert_eq!(anonymous, landed);
        assert_eq!(landed.landing(), Some(landing));
        assert_eq!(landed.clone().landing(), Some(landing));
        assert_eq!(landed.negated().landing(), Some(landing));
        assert_eq!(landed.negated().value_i64(), Some(-5));
        assert_eq!(anonymous.landing(), None);
        assert!(!LandedIntegerType::U64.is_signed());
        assert_eq!(LandedIntegerType::U64.bit_width(), 64);
        assert!(LandedIntegerType::Fixed(FixedInteger::signed(8)).is_signed());
    }

    #[test]
    fn integer_carrier_names_round_trip_and_bound_their_width() {
        /* Every width is a name; the conventional ones are not special. */
        let carriers = [1, 8, 13, 16, 32, 64, 65]
            .into_iter()
            .flat_map(|width| [FixedInteger::signed(width), FixedInteger::unsigned(width)])
            .map(LandedIntegerType::Fixed)
            .chain([LandedIntegerType::Addr]);
        for carrier in carriers {
            assert_eq!(LandedIntegerType::from_name(&carrier.name()), Some(carrier));
            let modulus = 1i128 << carrier.bit_width();
            let expected = if carrier.is_signed() {
                (-(modulus >> 1), (modulus >> 1) - 1)
            } else {
                (0, modulus - 1)
            };
            assert_eq!(carrier.bounds(), Some(expected));
        }
        assert_eq!(LandedIntegerType::Addr.name(), "addr");
        assert!(!LandedIntegerType::Addr.is_signed());
        for spelling in [
            "bool", "f32", "f64", "address", "usize", "U8", "u0", "i08", "u", "",
        ] {
            assert_eq!(LandedIntegerType::from_name(spelling), None);
        }
    }

    #[test]
    fn float_reads_round_directly_through_executable_semantics() {
        let witness = FloatLiteral::parse("8388609.499999999999999").unwrap();
        assert_eq!(witness.f32_bits(), 0x4b00_0001);

        let landed = FloatLiteral::parse("1.000000059604644775390625f32").unwrap();
        assert_eq!(landed.landing(), Some(FloatFormat::F32));
        assert_eq!(landed.value_f32().to_bits(), 1.0f32.to_bits());
        assert_eq!(landed.landed_f64().to_bits(), f64::from(1.0f32).to_bits());
    }

    #[test]
    fn decimal_consumers_preserve_their_spelling_window() {
        let decimal = IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "255").unwrap();
        let hexadecimal =
            IntegerLiteral::from_parts(false, IntegerRadix::Hexadecimal, "ff").unwrap();
        assert_ne!(decimal, hexadecimal);
        assert_eq!(decimal.value_i64(), hexadecimal.value_i64());
        assert_eq!(decimal.decimal_value_i128(), Some(255));
        assert_eq!(hexadecimal.decimal_value_i128(), None);
        assert_eq!(hexadecimal.decimal_value_bignum(), None);
        let maximum =
            IntegerLiteral::from_parts(false, IntegerRadix::Decimal, &i128::MAX.to_string())
                .unwrap();
        assert_eq!(maximum.decimal_value_i128(), Some(i128::MAX));
        let beyond = IntegerLiteral::from_parts(
            false,
            IntegerRadix::Decimal,
            "170141183460469231731687303715884105728",
        )
        .unwrap();
        assert_eq!(beyond.decimal_value_i128(), None);
        assert_eq!(beyond.negated().decimal_value_i128(), Some(i128::MIN));
    }

    #[test]
    fn oversized_magnitudes_still_validate_every_digit() {
        for radix in [
            IntegerRadix::Binary,
            IntegerRadix::Octal,
            IntegerRadix::Decimal,
            IntegerRadix::Hexadecimal,
        ] {
            let digits = "1".repeat(1024);
            let literal = IntegerLiteral::from_parts(false, radix, &digits).unwrap();
            assert_eq!(literal.to_string(), format!("{}{digits}", radix.prefix()));
            assert!(literal.value_bignum().is_some());
            assert_eq!(literal.negated().negated(), literal);
            assert!(IntegerLiteral::from_parts(false, radix, &format!("{digits}_z")).is_err());
        }
    }

    #[test]
    fn nondecimal_zero_negation_preserves_spelling_identity() {
        for radix in [
            IntegerRadix::Binary,
            IntegerRadix::Octal,
            IntegerRadix::Hexadecimal,
        ] {
            let positive = IntegerLiteral::from_parts(true, radix, "00_0").unwrap();
            let negative = positive.negated();
            assert!(!positive.is_negative());
            assert!(negative.is_negative());
            assert_eq!(negative.to_string(), format!("-{}0", radix.prefix()));
            assert_eq!(negative.value_i64(), Some(0));
            assert_eq!(negative.value_u64(), None);
            assert_ne!(positive, negative);
            assert_eq!(negative.negated(), positive);
        }
    }
}
