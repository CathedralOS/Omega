/*
We implement host-independent IEEE floating-point meanings (`FloatMeaning`,
`FloatSemantics`, `FloatFormat`) and the closed catalog of representation-free
semantic definitions (`FLOAT_SEMANTIC_OPERATIONS`, `FloatSemanticOperation`)
declared under `FloatSemantics::<name>` in the sealed toolchain source. Integer
conversions retain their sign and width in a parameterized operation identity,
so they do not allocate or search a catalog entry for each admitted width. Every
operation decodes its landed operands to exact rational or special-value
meanings, performs exact arithmetic, and rounds once through the selected
format record.
*/

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

use crate::bignum::{BigInt, BigRational, ExactFloat, IeeeRounding};
use crate::integer_carrier::FixedInteger;

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct FloatSpecialValues {
    pub signed_zero: bool,
    pub subnormals: bool,
    pub infinity: bool,
    pub nan: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct FloatFormat {
    pub radix: u32,
    pub precision: u32,
    pub minimum_normal_exponent: i32,
    pub maximum_normal_exponent: i32,
    pub minimum_subnormal_exponent: i32,
    pub specials: FloatSpecialValues,
    pub rounds_to_nearest_ties_to_even: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundingDirection {
    NearestTiesToEven,
    TowardZero,
    TowardPositive,
    TowardNegative,
}

impl RoundingDirection {
    fn ieee(self) -> IeeeRounding {
        match self {
            Self::NearestTiesToEven => IeeeRounding::NearestTiesToEven,
            Self::TowardZero => IeeeRounding::TowardZero,
            Self::TowardPositive => IeeeRounding::TowardPositive,
            Self::TowardNegative => IeeeRounding::TowardNegative,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatClass {
    NaN,
    Infinity { negative: bool },
    Normal { negative: bool },
    Subnormal { negative: bool },
    Zero { negative: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatToIntegerError {
    NonFinite,
    OutOfRange,
    MaterializationLimit {
        required_bits: u64,
        maximum_bits: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatPolicyTrap {
    NaNResult,
    InfinityResult,
}

impl FloatPolicyTrap {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NaNResult => "NaN",
            Self::InfinityResult => "infinity",
        }
    }
}

impl FloatFormat {
    pub const BINARY32: Self = Self {
        radix: 2,
        precision: 24,
        minimum_normal_exponent: -126,
        maximum_normal_exponent: 127,
        minimum_subnormal_exponent: -149,
        specials: FloatSpecialValues {
            signed_zero: true,
            subnormals: true,
            infinity: true,
            nan: true,
        },
        rounds_to_nearest_ties_to_even: true,
    };

    pub const BINARY64: Self = Self {
        radix: 2,
        precision: 53,
        minimum_normal_exponent: -1022,
        maximum_normal_exponent: 1023,
        minimum_subnormal_exponent: -1074,
        specials: FloatSpecialValues {
            signed_zero: true,
            subnormals: true,
            infinity: true,
            nan: true,
        },
        rounds_to_nearest_ties_to_even: true,
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FloatMeaning {
    FiniteNonZero(BigRational),
    Zero { negative: bool },
    Infinity { negative: bool },
    NaN,
}

macro_rules! rounded_binary {
    ($($name:ident => ($operation:ident, $direction:ident)),+ $(,)?) => {$(
        pub fn $name(
            format: FloatFormat,
            left: &FloatMeaning,
            right: &FloatMeaning,
        ) -> FloatMeaning {
            Self::round_binary_directed(
                format,
                left,
                right,
                ExactFloat::$operation,
                RoundingDirection::$direction,
            )
        }
    )+};
}

macro_rules! rounded_fused {
    ($($name:ident => $direction:ident),+ $(,)?) => {$(
        pub fn $name(
            format: FloatFormat,
            left: &FloatMeaning,
            right: &FloatMeaning,
            addend: &FloatMeaning,
        ) -> FloatMeaning {
            Self::fused_multiply_add_directed(
                format,
                left,
                right,
                addend,
                RoundingDirection::$direction,
            )
        }
    )+};
}

impl FloatMeaning {
    pub fn from_f32(value: f32) -> Self {
        Self::from_exact(ExactFloat::from_f32(value))
    }

    pub fn from_f64(value: f64) -> Self {
        Self::from_exact(ExactFloat::from_f64(value))
    }

    pub fn to_f32(&self) -> f32 {
        self.to_exact().to_f32()
    }

    pub fn to_f64(&self) -> f64 {
        self.to_exact().to_f64()
    }

    pub fn to_interpreter_value(&self, format: FloatFormat) -> f64 {
        if format == FloatFormat::BINARY32 {
            f64::from(self.to_f32())
        } else if format == FloatFormat::BINARY64 {
            self.to_f64()
        } else {
            panic!("unsupported floating-point format record: {format:?}")
        }
    }

    pub fn is_finite(&self) -> bool {
        matches!(self, Self::FiniteNonZero(_) | Self::Zero { .. })
    }

    pub fn is_nan(&self) -> bool {
        matches!(self, Self::NaN)
    }

    pub fn is_infinite(&self) -> bool {
        matches!(self, Self::Infinity { .. })
    }

    pub fn is_zero(&self) -> bool {
        matches!(self, Self::Zero { .. })
    }

    pub fn is_negative(&self) -> bool {
        match self {
            Self::FiniteNonZero(value) => value.is_negative(),
            Self::Zero { negative } | Self::Infinity { negative } => *negative,
            Self::NaN => false,
        }
    }

    fn from_exact(value: ExactFloat) -> Self {
        match value {
            ExactFloat::Finite(value) if value.is_zero() => Self::Zero {
                negative: value.is_negative(),
            },
            ExactFloat::Finite(value) => Self::FiniteNonZero(value),
            ExactFloat::Infinity { negative } => Self::Infinity { negative },
            ExactFloat::NaN => Self::NaN,
        }
    }

    fn to_exact(&self) -> ExactFloat {
        match self {
            Self::FiniteNonZero(value) => ExactFloat::Finite(value.clone()),
            Self::Zero { negative } => {
                ExactFloat::from_decimal_str(if *negative { "-0.0" } else { "0.0" })
                    .expect("zero spelling is exact")
            }
            Self::Infinity { negative } => ExactFloat::Infinity {
                negative: *negative,
            },
            Self::NaN => ExactFloat::NaN,
        }
    }
}

pub struct FloatSemantics;

impl FloatSemantics {
    pub fn from_decimal(format: FloatFormat, text: &str) -> Option<FloatMeaning> {
        Some(Self::round_exact(
            format,
            ExactFloat::from_decimal_str(text)?,
        ))
    }

    pub fn round_exact(format: FloatFormat, value: ExactFloat) -> FloatMeaning {
        Self::round_exact_directed(format, value, RoundingDirection::NearestTiesToEven)
    }

    pub fn round_exact_directed(
        format: FloatFormat,
        value: ExactFloat,
        direction: RoundingDirection,
    ) -> FloatMeaning {
        if format == FloatFormat::BINARY32 {
            FloatMeaning::from_f32(value.to_f32_with_rounding(direction.ieee()))
        } else if format == FloatFormat::BINARY64 {
            FloatMeaning::from_f64(value.to_f64_with_rounding(direction.ieee()))
        } else {
            panic!("unsupported floating-point format record: {format:?}")
        }
    }

    pub fn convert(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::round_exact(format, value.to_exact())
    }

    pub fn add(format: FloatFormat, left: &FloatMeaning, right: &FloatMeaning) -> FloatMeaning {
        Self::round_exact(format, left.to_exact().add(&right.to_exact()))
    }
    rounded_binary! {
        add_toward_zero => (add, TowardZero),
        add_toward_positive => (add, TowardPositive),
        add_toward_negative => (add, TowardNegative),
        subtract_toward_zero => (sub, TowardZero),
        subtract_toward_positive => (sub, TowardPositive),
        subtract_toward_negative => (sub, TowardNegative),
        multiply_toward_zero => (mul, TowardZero),
        multiply_toward_positive => (mul, TowardPositive),
        multiply_toward_negative => (mul, TowardNegative),
        divide_toward_zero => (div, TowardZero),
        divide_toward_positive => (div, TowardPositive),
        divide_toward_negative => (div, TowardNegative),
    }

    pub fn from_integer(format: FloatFormat, value: &BigInt) -> FloatMeaning {
        Self::round_exact(
            format,
            ExactFloat::Finite(BigRational::from_integer(value.clone())),
        )
    }

    pub fn from_integer_toward_zero(format: FloatFormat, value: &BigInt) -> FloatMeaning {
        Self::from_integer_directed(format, value, RoundingDirection::TowardZero)
    }

    pub fn to_integer_exact(
        value: &FloatMeaning,
        target: FixedInteger,
    ) -> Result<BigInt, FloatToIntegerError> {
        Self::checked_integer_result(value, target)
    }

    pub fn to_integer_trapping(
        value: &FloatMeaning,
        target: FixedInteger,
    ) -> Result<BigInt, FloatToIntegerError> {
        Self::checked_integer_result(value, target)
    }

    /// Builds a saturation endpoint only within the caller's endpoint bit budget.
    pub fn to_integer_saturating(
        value: &FloatMeaning,
        target: FixedInteger,
        maximum_bits: u64,
    ) -> Result<BigInt, FloatToIntegerError> {
        let truncated = match value {
            FloatMeaning::NaN | FloatMeaning::Zero { .. } => return Ok(BigInt::zero()),
            FloatMeaning::Infinity { negative } => {
                return Self::integer_endpoint(target, *negative, maximum_bits);
            }
            FloatMeaning::FiniteNonZero(value) => value.truncate_to_integer(),
        };
        if target.admits(&truncated) {
            Ok(truncated)
        } else {
            Self::integer_endpoint(target, truncated.is_negative(), maximum_bits)
        }
    }

    pub fn subtract(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_exact(format, left.to_exact().sub(&right.to_exact()))
    }

    pub fn multiply(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_exact(format, left.to_exact().mul(&right.to_exact()))
    }

    pub fn divide(format: FloatFormat, left: &FloatMeaning, right: &FloatMeaning) -> FloatMeaning {
        Self::round_exact(format, left.to_exact().div(&right.to_exact()))
    }

    pub fn negate(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::round_exact(format, value.to_exact().negate())
    }

    pub fn square_root(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::square_root_directed(format, value, RoundingDirection::NearestTiesToEven)
    }

    pub fn square_root_toward_zero(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::square_root_directed(format, value, RoundingDirection::TowardZero)
    }

    pub fn square_root_toward_positive(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::square_root_directed(format, value, RoundingDirection::TowardPositive)
    }

    pub fn square_root_toward_negative(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::square_root_directed(format, value, RoundingDirection::TowardNegative)
    }

    pub fn multiply_then_add(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
        addend: &FloatMeaning,
    ) -> FloatMeaning {
        let product = Self::multiply(format, left, right);
        Self::add(format, &product, addend)
    }

    pub fn fused_multiply_add(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
        addend: &FloatMeaning,
    ) -> FloatMeaning {
        let exact_product = left.to_exact().mul(&right.to_exact());
        Self::round_exact(format, exact_product.add(&addend.to_exact()))
    }
    rounded_fused! {
        fused_multiply_add_toward_zero => TowardZero,
        fused_multiply_add_toward_positive => TowardPositive,
        fused_multiply_add_toward_negative => TowardNegative,
    }

    pub fn classify(format: FloatFormat, value: &FloatMeaning) -> FloatClass {
        match value {
            FloatMeaning::NaN => FloatClass::NaN,
            FloatMeaning::Infinity { negative } => FloatClass::Infinity {
                negative: *negative,
            },
            FloatMeaning::Zero { negative } => FloatClass::Zero {
                negative: *negative,
            },
            FloatMeaning::FiniteNonZero(_) => {
                let (subnormal, negative) = if format == FloatFormat::BINARY32 {
                    let value = value.to_f32();
                    (value.is_subnormal(), value.is_sign_negative())
                } else if format == FloatFormat::BINARY64 {
                    let value = value.to_f64();
                    (value.is_subnormal(), value.is_sign_negative())
                } else {
                    panic!("unsupported floating-point format record: {format:?}")
                };
                if subnormal {
                    FloatClass::Subnormal { negative }
                } else {
                    FloatClass::Normal { negative }
                }
            }
        }
    }

    pub fn is_finite(value: &FloatMeaning) -> bool {
        value.is_finite()
    }

    pub fn is_nan(value: &FloatMeaning) -> bool {
        value.is_nan()
    }

    pub fn is_infinite(value: &FloatMeaning) -> bool {
        value.is_infinite()
    }

    pub fn is_normal(format: FloatFormat, value: &FloatMeaning) -> bool {
        matches!(Self::classify(format, value), FloatClass::Normal { .. })
    }

    pub fn is_subnormal(format: FloatFormat, value: &FloatMeaning) -> bool {
        matches!(Self::classify(format, value), FloatClass::Subnormal { .. })
    }

    pub fn apply_trapping_policy(result: FloatMeaning) -> Result<FloatMeaning, FloatPolicyTrap> {
        match result {
            FloatMeaning::NaN => Err(FloatPolicyTrap::NaNResult),
            FloatMeaning::Infinity { .. } => Err(FloatPolicyTrap::InfinityResult),
            finite => Ok(finite),
        }
    }

    pub fn apply_saturating_policy(
        format: FloatFormat,
        operands: &[&FloatMeaning],
        result: FloatMeaning,
    ) -> FloatMeaning {
        if result.is_infinite() && operands.iter().all(|operand| operand.is_finite()) {
            Self::maximum_finite(format, result.is_negative())
        } else {
            result
        }
    }

    pub fn apply_saturating_divide_policy(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
        result: FloatMeaning,
    ) -> FloatMeaning {
        if right.is_zero() {
            result
        } else {
            Self::apply_saturating_policy(format, &[left, right], result)
        }
    }

    pub fn equal(left: &FloatMeaning, right: &FloatMeaning) -> bool {
        left.to_exact().equal_value(&right.to_exact())
    }

    pub fn not_equal(left: &FloatMeaning, right: &FloatMeaning) -> bool {
        !Self::equal(left, right)
    }

    pub fn less(left: &FloatMeaning, right: &FloatMeaning) -> bool {
        left.to_exact().partial_cmp_value(&right.to_exact()) == Some(Ordering::Less)
    }

    pub fn less_or_equal(left: &FloatMeaning, right: &FloatMeaning) -> bool {
        matches!(
            left.to_exact().partial_cmp_value(&right.to_exact()),
            Some(Ordering::Less | Ordering::Equal)
        )
    }

    pub fn greater(left: &FloatMeaning, right: &FloatMeaning) -> bool {
        left.to_exact().partial_cmp_value(&right.to_exact()) == Some(Ordering::Greater)
    }

    pub fn greater_or_equal(left: &FloatMeaning, right: &FloatMeaning) -> bool {
        matches!(
            left.to_exact().partial_cmp_value(&right.to_exact()),
            Some(Ordering::Greater | Ordering::Equal)
        )
    }

    pub fn maximum_finite(format: FloatFormat, negative: bool) -> FloatMeaning {
        if format == FloatFormat::BINARY32 {
            let sign = if negative { 0x8000_0000 } else { 0 };
            FloatMeaning::from_f32(f32::from_bits(sign | 0x7f7f_ffff))
        } else if format == FloatFormat::BINARY64 {
            let sign = if negative { 0x8000_0000_0000_0000 } else { 0 };
            FloatMeaning::from_f64(f64::from_bits(sign | 0x7fef_ffff_ffff_ffff))
        } else {
            panic!("unsupported floating-point format record: {format:?}")
        }
    }

    pub fn minimum(left: &FloatMeaning, right: &FloatMeaning) -> FloatMeaning {
        if Self::less(left, right) {
            left.clone()
        } else {
            right.clone()
        }
    }

    pub fn maximum(left: &FloatMeaning, right: &FloatMeaning) -> FloatMeaning {
        if Self::greater(left, right) {
            left.clone()
        } else {
            right.clone()
        }
    }

    fn round_binary_directed(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
        operation: fn(&ExactFloat, &ExactFloat) -> ExactFloat,
        direction: RoundingDirection,
    ) -> FloatMeaning {
        let exact = operation(&left.to_exact(), &right.to_exact());
        Self::round_exact_directed(format, exact, direction)
    }

    fn from_integer_directed(
        format: FloatFormat,
        value: &BigInt,
        direction: RoundingDirection,
    ) -> FloatMeaning {
        Self::round_exact_directed(
            format,
            ExactFloat::Finite(BigRational::from_integer(value.clone())),
            direction,
        )
    }

    // Only saturation needs a carrier endpoint; range checking itself never builds one.
    fn integer_endpoint(
        target: FixedInteger,
        minimum: bool,
        maximum_bits: u64,
    ) -> Result<BigInt, FloatToIntegerError> {
        if minimum && !target.is_signed() {
            return Ok(BigInt::zero());
        }
        let exponent = target.width() - u64::from(target.is_signed());
        let required_bits = if minimum { target.width() } else { exponent };
        if required_bits > maximum_bits || usize::try_from(exponent).is_err() {
            return Err(FloatToIntegerError::MaterializationLimit {
                required_bits,
                maximum_bits,
            });
        }
        if required_bits == 0 {
            return Ok(BigInt::zero());
        }
        let power = BigInt::from_u64(1).shl_bits(exponent as usize);
        Ok(if minimum {
            power.negate()
        } else {
            power.sub(&BigInt::from_u64(1))
        })
    }

    fn checked_integer_result(
        value: &FloatMeaning,
        target: FixedInteger,
    ) -> Result<BigInt, FloatToIntegerError> {
        let truncated = match value {
            FloatMeaning::FiniteNonZero(value) => value.truncate_to_integer(),
            FloatMeaning::Zero { .. } => BigInt::zero(),
            FloatMeaning::Infinity { .. } | FloatMeaning::NaN => {
                return Err(FloatToIntegerError::NonFinite);
            }
        };
        if !target.admits(&truncated) {
            Err(FloatToIntegerError::OutOfRange)
        } else {
            Ok(truncated)
        }
    }

    fn square_root_directed(
        format: FloatFormat,
        value: &FloatMeaning,
        direction: RoundingDirection,
    ) -> FloatMeaning {
        let input = match value {
            FloatMeaning::NaN | FloatMeaning::Infinity { negative: true } => {
                return FloatMeaning::NaN;
            }
            FloatMeaning::Infinity { negative: false } => {
                return FloatMeaning::Infinity { negative: false };
            }
            FloatMeaning::Zero { negative } => {
                return FloatMeaning::Zero {
                    negative: *negative,
                };
            }
            FloatMeaning::FiniteNonZero(value) if value.is_negative() => {
                return FloatMeaning::NaN;
            }
            FloatMeaning::FiniteNonZero(value) => value,
        };

        if format == FloatFormat::BINARY32 {
            let bits = correctly_rounded_square_root_bits(
                input,
                u64::from(f32::MAX.to_bits()),
                |bits| match ExactFloat::from_f32(f32::from_bits(bits as u32)) {
                    ExactFloat::Finite(value) => value,
                    _ => unreachable!("positive finite binary32 bits decode to a rational"),
                },
                direction,
            );
            FloatMeaning::from_f32(f32::from_bits(bits as u32))
        } else if format == FloatFormat::BINARY64 {
            let bits = correctly_rounded_square_root_bits(
                input,
                f64::MAX.to_bits(),
                |bits| match ExactFloat::from_f64(f64::from_bits(bits)) {
                    ExactFloat::Finite(value) => value,
                    _ => unreachable!("positive finite binary64 bits decode to a rational"),
                },
                direction,
            );
            FloatMeaning::from_f64(f64::from_bits(bits))
        } else {
            panic!("unsupported floating-point format record: {format:?}")
        }
    }

    fn fused_multiply_add_directed(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
        addend: &FloatMeaning,
        direction: RoundingDirection,
    ) -> FloatMeaning {
        let exact_product = left.to_exact().mul(&right.to_exact());
        Self::round_exact_directed(format, exact_product.add(&addend.to_exact()), direction)
    }
}

fn correctly_rounded_square_root_bits(
    input: &BigRational,
    maximum_finite_bits: u64,
    decode: impl Fn(u64) -> BigRational,
    direction: RoundingDirection,
) -> u64 {
    let mut lower = 0u64;
    let mut upper = maximum_finite_bits;
    while lower < upper {
        let middle = lower + (upper - lower).div_ceil(2);
        let candidate = decode(middle);
        if candidate.mul(&candidate).cmp_value(input) != Ordering::Greater {
            lower = middle;
        } else {
            upper = middle - 1;
        }
    }

    let floor = decode(lower);
    if floor.mul(&floor).cmp_value(input) == Ordering::Equal {
        return lower;
    }
    let ceiling_bits = lower + 1;
    if matches!(
        direction,
        RoundingDirection::TowardZero | RoundingDirection::TowardNegative
    ) {
        return lower;
    }
    if direction == RoundingDirection::TowardPositive {
        return ceiling_bits;
    }

    let ceiling = decode(ceiling_bits);
    let four_input = input.mul(&BigRational::from_integer(BigInt::from_u64(4)));
    let sum = floor.add(&ceiling);
    match four_input.cmp_value(&sum.mul(&sum)) {
        Ordering::Less => lower,
        Ordering::Greater => ceiling_bits,
        Ordering::Equal if lower & 1 == 0 => lower,
        Ordering::Equal => ceiling_bits,
    }
}

pub const FLOAT_SEMANTICS_CATALOG_VERSION: u16 = 2;
pub const FLOAT_SEMANTICS_NAMESPACE: &str = "FloatSemantics";
pub const FLOAT_FORMAT_CORE_SOURCE: &str = "float_format.omg";
const FLOAT_SEMANTICS_CONTRACT_DOMAIN: &[u8] = b"psi-float-semantics-contract\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatSemanticValueKind {
    Format,
    Meaning,
    Class,
    Bool,
    Integer(FixedInteger),
}

impl FloatSemanticValueKind {
    pub fn from_spelling(spelling: &str) -> Option<Self> {
        Some(match spelling {
            "FloatFormat" => Self::Format,
            "FloatMeaning" => Self::Meaning,
            "FloatClass" => Self::Class,
            "bool" => Self::Bool,
            other => Self::Integer(FixedInteger::from_decimal_spelling(other)?),
        })
    }
}

impl fmt::Display for FloatSemanticValueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format => f.write_str("FloatFormat"),
            Self::Meaning => f.write_str("FloatMeaning"),
            Self::Class => f.write_str("FloatClass"),
            Self::Bool => f.write_str("bool"),
            Self::Integer(carrier) => carrier.fmt(f),
        }
    }
}

/// Conversion identity retains the carrier, not an index into a finite width roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum FloatSemanticOperationIdentity {
    Fixed(u8),
    FromInteger(FixedInteger),
    ToIntegerExact(FixedInteger),
    ToIntegerSaturating(FixedInteger),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatSemanticName {
    Fixed(&'static str),
    FromInteger,
    ToIntegerExact(FixedInteger),
    ToIntegerSaturating(FixedInteger),
}

impl FloatSemanticName {
    fn matches_source(self, name: &str) -> bool {
        match self {
            Self::Fixed(fixed) => fixed == name,
            Self::FromInteger => name == "from_integer",
            Self::ToIntegerExact(carrier) => {
                name.strip_prefix("to_")
                    .and_then(FixedInteger::from_decimal_spelling)
                    == Some(carrier)
            }
            Self::ToIntegerSaturating(carrier) => {
                name.strip_prefix("to_")
                    .and_then(|name| name.strip_suffix("_saturating"))
                    .and_then(FixedInteger::from_decimal_spelling)
                    == Some(carrier)
            }
        }
    }
}

impl fmt::Display for FloatSemanticName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fixed(name) => f.write_str(name),
            Self::FromInteger => f.write_str("from_integer"),
            Self::ToIntegerExact(carrier) => write!(f, "to_{carrier}"),
            Self::ToIntegerSaturating(carrier) => write!(f, "to_{carrier}_saturating"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct FloatSemanticContractIdentity {
    pub operation: FloatSemanticOperationIdentity,
    pub catalog_version: u16,
    pub commitment: [u8; 32],
}

#[derive(Debug, Clone, Copy)]
pub struct FloatSemanticOperation {
    pub name: FloatSemanticName,
    parameters: [FloatSemanticValueKind; 4],
    parameter_count: u8,
    pub result: FloatSemanticValueKind,
    kernel: FloatSemanticKernel,
}

impl PartialEq for FloatSemanticOperation {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.parameters() == other.parameters()
            && self.result == other.result
    }
}

impl Eq for FloatSemanticOperation {}

impl PartialOrd for FloatSemanticOperation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FloatSemanticOperation {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name
            .cmp(&other.name)
            .then_with(|| self.parameters().cmp(other.parameters()))
            .then_with(|| self.result.cmp(&other.result))
    }
}

impl Hash for FloatSemanticOperation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.parameters().hash(state);
        self.result.hash(state);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FloatSemanticOperand {
    Format(FloatFormat),
    Meaning(FloatMeaning),
    Integer(BigInt),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FloatSemanticResult {
    Meaning(FloatMeaning),
    Class(FloatClass),
    Bool(bool),
    Integer(BigInt),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FloatSemanticDischarge {
    pub contract: FloatSemanticContractIdentity,
    pub result: FloatSemanticResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatSemanticEvaluationError {
    OperandSignatureMismatch,
    UnsupportedApplication,
    IntegerConversion(FloatToIntegerError),
    ResultSignatureMismatch,
}

#[derive(Debug, Clone, Copy)]
pub enum FloatSemanticKernel {
    BinaryMeaning(fn(FloatFormat, &FloatMeaning, &FloatMeaning) -> FloatMeaning),
    UnaryMeaning(fn(FloatFormat, &FloatMeaning) -> FloatMeaning),
    TernaryMeaning(fn(FloatFormat, &FloatMeaning, &FloatMeaning, &FloatMeaning) -> FloatMeaning),
    FromInteger(fn(FloatFormat, &BigInt) -> FloatMeaning),
    ToIntegerExact,
    ToIntegerSaturating,
    PairMeaning(fn(&FloatMeaning, &FloatMeaning) -> FloatMeaning),
    PairBool(FloatSemanticComparison),
    UnaryClass(fn(FloatFormat, &FloatMeaning) -> FloatClass),
    SinglePredicate(FloatSemanticPredicate),
    UnaryBool(fn(FloatFormat, &FloatMeaning) -> bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatSemanticComparison {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl FloatSemanticComparison {
    fn evaluate(self, left: &FloatMeaning, right: &FloatMeaning) -> bool {
        match self {
            Self::Equal => FloatSemantics::equal(left, right),
            Self::NotEqual => FloatSemantics::not_equal(left, right),
            Self::Less => FloatSemantics::less(left, right),
            Self::LessOrEqual => FloatSemantics::less_or_equal(left, right),
            Self::Greater => FloatSemantics::greater(left, right),
            Self::GreaterOrEqual => FloatSemantics::greater_or_equal(left, right),
        }
    }

    pub const fn unordered_truth(self) -> bool {
        matches!(self, Self::NotEqual)
    }

    pub const fn same_meaning_truth(self, nan: Option<bool>) -> Option<bool> {
        match self {
            Self::Less | Self::Greater => Some(false),
            _ => match nan {
                Some(true) => Some(self.unordered_truth()),
                Some(false) => Some(!matches!(self, Self::NotEqual)),
                None => None,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatSemanticPredicate {
    Finite,
    NaN,
    Infinite,
}

impl FloatSemanticPredicate {
    fn evaluate(self, value: &FloatMeaning) -> bool {
        match self {
            Self::Finite => FloatSemantics::is_finite(value),
            Self::NaN => FloatSemantics::is_nan(value),
            Self::Infinite => FloatSemantics::is_infinite(value),
        }
    }

    fn ordered_interval(self, minimum: &FloatMeaning, maximum: &FloatMeaning) -> Option<bool> {
        if !FloatSemantics::less_or_equal(minimum, maximum) {
            return None;
        }
        match self {
            Self::NaN => Some(false),
            Self::Finite | Self::Infinite if minimum.is_finite() && maximum.is_finite() => {
                Some(self == Self::Finite)
            }
            Self::Finite | Self::Infinite if minimum == maximum => Some(self.evaluate(minimum)),
            _ => None,
        }
    }
}

impl FloatSemanticOperand {
    fn matches_kind(&self, kind: FloatSemanticValueKind) -> bool {
        match (self, kind) {
            (Self::Format(format), FloatSemanticValueKind::Format) => {
                *format == FloatFormat::BINARY32 || *format == FloatFormat::BINARY64
            }
            (Self::Meaning(_), FloatSemanticValueKind::Meaning) => true,
            (Self::Integer(value), FloatSemanticValueKind::Integer(carrier)) => {
                carrier.admits(value)
            }
            _ => false,
        }
    }

    fn as_format(&self) -> Option<FloatFormat> {
        match self {
            Self::Format(format) => Some(*format),
            _ => None,
        }
    }

    fn as_meaning(&self) -> Option<&FloatMeaning> {
        match self {
            Self::Meaning(meaning) => Some(meaning),
            _ => None,
        }
    }

    fn as_integer(&self) -> Option<&BigInt> {
        match self {
            Self::Integer(value) => Some(value),
            _ => None,
        }
    }
}

impl FloatSemanticResult {
    fn matches_kind(&self, kind: FloatSemanticValueKind) -> bool {
        match (self, kind) {
            (Self::Meaning(_), FloatSemanticValueKind::Meaning) => true,
            (Self::Class(_), FloatSemanticValueKind::Class) => true,
            (Self::Bool(_), FloatSemanticValueKind::Bool) => true,
            (Self::Integer(value), FloatSemanticValueKind::Integer(carrier)) => {
                carrier.admits(value)
            }
            _ => false,
        }
    }
}

impl FloatSemanticKernel {
    fn evaluate(
        self,
        result_kind: FloatSemanticValueKind,
        arguments: &[FloatSemanticOperand],
        maximum_endpoint_bits: u64,
    ) -> Result<FloatSemanticResult, FloatSemanticEvaluationError> {
        if matches!(self, Self::ToIntegerExact | Self::ToIntegerSaturating) {
            let [FloatSemanticOperand::Meaning(value)] = arguments else {
                return Err(FloatSemanticEvaluationError::OperandSignatureMismatch);
            };
            let FloatSemanticValueKind::Integer(carrier) = result_kind else {
                return Err(FloatSemanticEvaluationError::ResultSignatureMismatch);
            };
            return match self {
                Self::ToIntegerExact => FloatSemantics::to_integer_exact(value, carrier),
                _ => FloatSemantics::to_integer_saturating(value, carrier, maximum_endpoint_bits),
            }
            .map(FloatSemanticResult::Integer)
            .map_err(FloatSemanticEvaluationError::IntegerConversion);
        }
        let result = (|| match self {
            Self::BinaryMeaning(kernel) => {
                let [format, left, right] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Meaning(kernel(
                    format.as_format()?,
                    left.as_meaning()?,
                    right.as_meaning()?,
                )))
            }
            Self::UnaryMeaning(kernel) => {
                let [format, value] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Meaning(kernel(
                    format.as_format()?,
                    value.as_meaning()?,
                )))
            }
            Self::TernaryMeaning(kernel) => {
                let [format, left, right, addend] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Meaning(kernel(
                    format.as_format()?,
                    left.as_meaning()?,
                    right.as_meaning()?,
                    addend.as_meaning()?,
                )))
            }
            Self::FromInteger(kernel) => {
                let [format, value] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Meaning(kernel(
                    format.as_format()?,
                    value.as_integer()?,
                )))
            }
            Self::ToIntegerExact | Self::ToIntegerSaturating => None,
            Self::PairMeaning(kernel) => {
                let [left, right] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Meaning(kernel(
                    left.as_meaning()?,
                    right.as_meaning()?,
                )))
            }
            Self::PairBool(kernel) => {
                let [left, right] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Bool(
                    kernel.evaluate(left.as_meaning()?, right.as_meaning()?),
                ))
            }
            Self::UnaryClass(kernel) => {
                let [format, value] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Class(kernel(
                    format.as_format()?,
                    value.as_meaning()?,
                )))
            }
            Self::SinglePredicate(predicate) => {
                let [value] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Bool(
                    predicate.evaluate(value.as_meaning()?),
                ))
            }
            Self::UnaryBool(kernel) => {
                let [format, value] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Bool(kernel(
                    format.as_format()?,
                    value.as_meaning()?,
                )))
            }
        })();
        result.ok_or(FloatSemanticEvaluationError::UnsupportedApplication)
    }
}

use FloatSemanticKernel::{
    BinaryMeaning, FromInteger, PairBool, PairMeaning, SinglePredicate, TernaryMeaning,
    ToIntegerExact, ToIntegerSaturating, UnaryBool, UnaryClass, UnaryMeaning,
};
use FloatSemanticValueKind::{Bool, Class, Format, Meaning};

const fn integer(carrier: FixedInteger) -> FloatSemanticValueKind {
    FloatSemanticValueKind::Integer(carrier)
}

const fn row(
    name: &'static str,
    parameters: &'static [FloatSemanticValueKind],
    result: FloatSemanticValueKind,
    kernel: FloatSemanticKernel,
) -> FloatSemanticOperation {
    let mut retained = [Meaning; 4];
    let mut index = 0;
    while index < parameters.len() {
        retained[index] = parameters[index];
        index += 1;
    }
    FloatSemanticOperation {
        name: FloatSemanticName::Fixed(name),
        parameters: retained,
        parameter_count: parameters.len() as u8,
        result,
        kernel,
    }
}

const BINARY: &[FloatSemanticValueKind] = &[Format, Meaning, Meaning];
const UNARY: &[FloatSemanticValueKind] = &[Format, Meaning];
const TERNARY: &[FloatSemanticValueKind] = &[Format, Meaning, Meaning, Meaning];
const PAIR: &[FloatSemanticValueKind] = &[Meaning, Meaning];
const SINGLE: &[FloatSemanticValueKind] = &[Meaning];

macro_rules! binary {
    ($name:ident) => {
        row(
            stringify!($name),
            BINARY,
            Meaning,
            BinaryMeaning(FloatSemantics::$name),
        )
    };
}
macro_rules! unary {
    ($name:ident) => {
        row(
            stringify!($name),
            UNARY,
            Meaning,
            UnaryMeaning(FloatSemantics::$name),
        )
    };
}
macro_rules! ternary {
    ($name:ident) => {
        row(
            stringify!($name),
            TERNARY,
            Meaning,
            TernaryMeaning(FloatSemantics::$name),
        )
    };
}
macro_rules! pair {
    ($name:ident) => {
        row(
            stringify!($name),
            PAIR,
            Meaning,
            PairMeaning(FloatSemantics::$name),
        )
    };
}
macro_rules! unary_bool {
    ($name:ident) => {
        row(
            stringify!($name),
            UNARY,
            Bool,
            UnaryBool(FloatSemantics::$name),
        )
    };
}
macro_rules! compare {
    ($name:literal, $relation:ident) => {
        row(
            $name,
            PAIR,
            Bool,
            PairBool(FloatSemanticComparison::$relation),
        )
    };
}
macro_rules! predicate {
    ($name:literal, $kind:ident) => {
        row(
            $name,
            SINGLE,
            Bool,
            SinglePredicate(FloatSemanticPredicate::$kind),
        )
    };
}

pub const FLOAT_SEMANTIC_OPERATIONS: &[FloatSemanticOperation] = &[
    binary!(add),
    binary!(subtract),
    binary!(multiply),
    binary!(divide),
    unary!(negate),
    unary!(square_root),
    unary!(convert),
    ternary!(multiply_then_add),
    ternary!(fused_multiply_add),
    pair!(minimum),
    pair!(maximum),
    compare!("equal", Equal),
    compare!("not_equal", NotEqual),
    compare!("less", Less),
    compare!("less_or_equal", LessOrEqual),
    compare!("greater", Greater),
    compare!("greater_or_equal", GreaterOrEqual),
    row(
        "classify",
        UNARY,
        Class,
        UnaryClass(FloatSemantics::classify),
    ),
    predicate!("is_finite", Finite),
    predicate!("is_nan", NaN),
    predicate!("is_infinite", Infinite),
    unary_bool!(is_normal),
    unary_bool!(is_subnormal),
    binary!(add_toward_zero),
    binary!(add_toward_positive),
    binary!(add_toward_negative),
    binary!(subtract_toward_zero),
    binary!(subtract_toward_positive),
    binary!(subtract_toward_negative),
    binary!(multiply_toward_zero),
    binary!(multiply_toward_positive),
    binary!(multiply_toward_negative),
    binary!(divide_toward_zero),
    binary!(divide_toward_positive),
    binary!(divide_toward_negative),
    unary!(square_root_toward_zero),
    unary!(square_root_toward_positive),
    unary!(square_root_toward_negative),
    ternary!(fused_multiply_add_toward_zero),
    ternary!(fused_multiply_add_toward_positive),
    ternary!(fused_multiply_add_toward_negative),
];

pub const FLOAT_NAMED_CALL_LEAVES: &[&str] = &[
    "negate",
    "square_root",
    "classify",
    "is_finite",
    "is_nan",
    "is_infinite",
    "is_normal",
    "is_subnormal",
    "square_root_toward_zero",
    "square_root_toward_positive",
    "square_root_toward_negative",
    "minimum",
    "maximum",
    "add_toward_zero",
    "add_toward_positive",
    "add_toward_negative",
    "subtract_toward_zero",
    "subtract_toward_positive",
    "subtract_toward_negative",
    "multiply_toward_zero",
    "multiply_toward_positive",
    "multiply_toward_negative",
    "divide_toward_zero",
    "divide_toward_positive",
    "divide_toward_negative",
    "multiply_then_add",
    "fused_multiply_add",
    "fused_multiply_add_toward_zero",
    "fused_multiply_add_toward_positive",
    "fused_multiply_add_toward_negative",
];

impl FloatSemanticOperation {
    pub fn parameters(&self) -> &[FloatSemanticValueKind] {
        &self.parameters[..usize::from(self.parameter_count)]
    }

    fn from_operation_identity(identity: FloatSemanticOperationIdentity) -> Option<Self> {
        let (name, parameters, parameter_count, result, kernel) = match identity {
            FloatSemanticOperationIdentity::Fixed(row) => {
                return FLOAT_SEMANTIC_OPERATIONS.get(usize::from(row)).copied();
            }
            FloatSemanticOperationIdentity::FromInteger(carrier) => (
                FloatSemanticName::FromInteger,
                [Format, integer(carrier), Meaning, Meaning],
                2,
                Meaning,
                FromInteger(FloatSemantics::from_integer),
            ),
            FloatSemanticOperationIdentity::ToIntegerExact(carrier) => (
                FloatSemanticName::ToIntegerExact(carrier),
                [Meaning; 4],
                1,
                integer(carrier),
                ToIntegerExact,
            ),
            FloatSemanticOperationIdentity::ToIntegerSaturating(carrier) => (
                FloatSemanticName::ToIntegerSaturating(carrier),
                [Meaning; 4],
                1,
                integer(carrier),
                ToIntegerSaturating,
            ),
        };
        Some(Self {
            name,
            parameters,
            parameter_count,
            result,
            kernel,
        })
    }

    pub fn identity(&self) -> FloatSemanticOperationIdentity {
        match self.name {
            FloatSemanticName::Fixed(_) => FloatSemanticOperationIdentity::Fixed(
                FLOAT_SEMANTIC_OPERATIONS
                    .iter()
                    .position(|row| row == self)
                    .and_then(|index| u8::try_from(index).ok())
                    .expect("closed float operation"),
            ),
            FloatSemanticName::FromInteger => {
                let FloatSemanticValueKind::Integer(carrier) = self.parameters()[1] else {
                    unreachable!("from_integer retains its input carrier")
                };
                FloatSemanticOperationIdentity::FromInteger(carrier)
            }
            FloatSemanticName::ToIntegerExact(carrier) => {
                FloatSemanticOperationIdentity::ToIntegerExact(carrier)
            }
            FloatSemanticName::ToIntegerSaturating(carrier) => {
                FloatSemanticOperationIdentity::ToIntegerSaturating(carrier)
            }
        }
    }

    pub fn namespace_matches(namespace: &str) -> bool {
        namespace == FLOAT_SEMANTICS_NAMESPACE
    }

    pub fn from_source_identity(
        namespace: &str,
        name: &str,
        parameters: &[FloatSemanticValueKind],
        result: FloatSemanticValueKind,
    ) -> Option<FloatSemanticOperation> {
        if !Self::namespace_matches(namespace) {
            return None;
        }
        if let Some(operation) = FLOAT_SEMANTIC_OPERATIONS.iter().find(|row| {
            row.name.matches_source(name) && row.parameters() == parameters && row.result == result
        }) {
            return Some(*operation);
        }
        let identity = match (name, parameters, result) {
            ("from_integer", [Format, FloatSemanticValueKind::Integer(carrier)], Meaning) => {
                FloatSemanticOperationIdentity::FromInteger(*carrier)
            }
            (_, [Meaning], FloatSemanticValueKind::Integer(carrier)) => {
                if FloatSemanticName::ToIntegerExact(carrier).matches_source(name) {
                    FloatSemanticOperationIdentity::ToIntegerExact(carrier)
                } else if FloatSemanticName::ToIntegerSaturating(carrier).matches_source(name) {
                    FloatSemanticOperationIdentity::ToIntegerSaturating(carrier)
                } else {
                    return None;
                }
            }
            _ => return None,
        };
        Self::from_operation_identity(identity)
    }

    pub fn names_a_row(namespace: &str, name: &str) -> bool {
        Self::namespace_matches(namespace)
            && (FLOAT_SEMANTIC_OPERATIONS
                .iter()
                .any(|row| row.name.matches_source(name))
                || name == "from_integer"
                || name
                    .strip_prefix("to_")
                    .and_then(|name| {
                        FixedInteger::from_decimal_spelling(
                            name.strip_suffix("_saturating").unwrap_or(name),
                        )
                    })
                    .is_some())
    }

    pub fn named_call_arity(name: &str) -> Option<usize> {
        if !FLOAT_NAMED_CALL_LEAVES.contains(&name) {
            return None;
        }
        FLOAT_SEMANTIC_OPERATIONS
            .iter()
            .find(|row| row.name.matches_source(name))
            .map(|row| {
                row.parameters().len()
                    - usize::from(row.parameters().first() == Some(&FloatSemanticValueKind::Format))
            })
    }

    pub const fn kernel(&self) -> FloatSemanticKernel {
        self.kernel
    }

    pub fn kernel_discharge(
        &self,
        arguments: &[FloatSemanticOperand],
        maximum_endpoint_bits: u64,
    ) -> Result<FloatSemanticDischarge, FloatSemanticEvaluationError> {
        if arguments.len() != self.parameters().len()
            || !arguments
                .iter()
                .zip(self.parameters().iter())
                .all(|(operand, kind)| operand.matches_kind(*kind))
        {
            return Err(FloatSemanticEvaluationError::OperandSignatureMismatch);
        }
        let result = self
            .kernel
            .evaluate(self.result, arguments, maximum_endpoint_bits)?;
        if !result.matches_kind(self.result) {
            return Err(FloatSemanticEvaluationError::ResultSignatureMismatch);
        }
        Ok(FloatSemanticDischarge {
            contract: self.contract_identity(),
            result,
        })
    }

    pub fn integer_operand_image(
        &self,
        format: FloatFormat,
        carrier: FixedInteger,
        minimum: &BigInt,
        maximum: &BigInt,
    ) -> Option<(FloatMeaning, FloatMeaning)> {
        if !matches!(self.kernel, FloatSemanticKernel::FromInteger(_))
            || self.result != FloatSemanticValueKind::Meaning
            || self.parameters()
                != [
                    FloatSemanticValueKind::Format,
                    FloatSemanticValueKind::Integer(carrier),
                ]
            || minimum > maximum
        {
            return None;
        }
        let endpoint = |value: &BigInt| {
            let discharge = self
                .kernel_discharge(
                    &[
                        FloatSemanticOperand::Format(format),
                        FloatSemanticOperand::Integer(value.clone()),
                    ],
                    0,
                )
                .ok()?;
            let FloatSemanticResult::Meaning(meaning) = discharge.result else {
                return None;
            };
            Some(meaning)
        };
        Some((endpoint(minimum)?, endpoint(maximum)?))
    }

    pub fn ordered_interval_predicate(
        &self,
        minimum: &FloatMeaning,
        maximum: &FloatMeaning,
    ) -> Option<bool> {
        if self.parameters() != [FloatSemanticValueKind::Meaning]
            || self.result != FloatSemanticValueKind::Bool
        {
            return None;
        }
        let FloatSemanticKernel::SinglePredicate(predicate) = self.kernel else {
            return None;
        };
        predicate.ordered_interval(minimum, maximum)
    }

    pub fn for_contract_identity(
        identity: &FloatSemanticContractIdentity,
    ) -> Option<FloatSemanticOperation> {
        if identity.catalog_version != FLOAT_SEMANTICS_CATALOG_VERSION {
            return None;
        }
        let row = Self::from_operation_identity(identity.operation)?;
        if row.contract_identity() == *identity {
            Some(row)
        } else {
            None
        }
    }

    pub fn contract_identity(&self) -> FloatSemanticContractIdentity {
        let mut hasher =
            crate::digest_encoder::DomainDigestEncoder::new(FLOAT_SEMANTICS_CONTRACT_DOMAIN);
        hasher.raw(&FLOAT_SEMANTICS_CATALOG_VERSION.to_le_bytes());
        hasher.raw(format!("toolchain::{}::{}", FLOAT_SEMANTICS_NAMESPACE, self.name).as_bytes());
        hasher.byte(0);
        for parameter in self.parameters() {
            hasher.raw(parameter.to_string().as_bytes());
            hasher.byte(0);
        }
        hasher.raw(b"->");
        hasher.raw(self.result.to_string().as_bytes());
        FloatSemanticContractIdentity {
            operation: self.identity(),
            catalog_version: FLOAT_SEMANTICS_CATALOG_VERSION,
            commitment: hasher.finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::bignum::{BigInt, ExactFloat};

    #[test]
    fn width_parameterized_conversion_catalog_accepts_exact_unusual_carriers() {
        let integer = super::FloatSemanticValueKind::from_spelling("u13")
            .expect("positive integer widths are semantic carriers");
        let operation = super::FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "from_integer",
            &[super::FloatSemanticValueKind::Format, integer],
            super::FloatSemanticValueKind::Meaning,
        )
        .expect("conversion selection is parameterized by its carrier");
        let identity = operation.contract_identity();
        assert_eq!(
            super::FloatSemanticOperation::for_contract_identity(&identity),
            Some(operation)
        );
        assert!(
            super::FloatSemanticOperation::from_source_identity(
                "FloatSemantics",
                "to_u13",
                &[super::FloatSemanticValueKind::Meaning],
                integer,
            )
            .is_some()
        );
        assert!(
            super::FloatSemanticOperation::from_source_identity(
                "FloatSemantics",
                "to_i13",
                &[super::FloatSemanticValueKind::Meaning],
                integer,
            )
            .is_none()
        );
    }

    #[test]
    fn width_parameterized_conversions_keep_validity_and_resource_refusal_distinct() {
        use super::{FloatSemanticEvaluationError, FloatSemanticOperationIdentity};
        for carrier in [
            FixedInteger::signed(1),
            FixedInteger::unsigned(13),
            FixedInteger::signed(257),
            FixedInteger::unsigned(u64::MAX),
        ] {
            for identity in [
                FloatSemanticOperationIdentity::FromInteger(carrier),
                FloatSemanticOperationIdentity::ToIntegerExact(carrier),
                FloatSemanticOperationIdentity::ToIntegerSaturating(carrier),
            ] {
                let operation = FloatSemanticOperation::from_operation_identity(identity).unwrap();
                let contract = operation.contract_identity();
                assert_eq!(
                    FloatSemanticOperation::for_contract_identity(&contract),
                    Some(operation)
                );
                let mut altered = contract;
                altered.operation =
                    FloatSemanticOperationIdentity::ToIntegerExact(FixedInteger::unsigned(7));
                assert!(FloatSemanticOperation::for_contract_identity(&altered).is_none());
                let arguments =
                    if matches!(identity, FloatSemanticOperationIdentity::FromInteger(_)) {
                        vec![
                            FloatSemanticOperand::Format(FloatFormat::BINARY64),
                            FloatSemanticOperand::Integer(BigInt::zero()),
                        ]
                    } else {
                        vec![FloatSemanticOperand::Meaning(FloatMeaning::from_f64(0.0))]
                    };
                operation
                    .kernel_discharge(&arguments, 0)
                    .expect("zero needs no endpoint");
            }
        }
        let huge = FixedInteger::unsigned(u64::MAX);
        assert_eq!(
            FloatSemantics::to_integer_exact(&FloatMeaning::from_f64(42.75), huge).unwrap(),
            BigInt::from_u64(42)
        );
        let saturated = FloatSemanticOperation::from_operation_identity(
            FloatSemanticOperationIdentity::ToIntegerSaturating(huge),
        )
        .unwrap();
        assert_eq!(
            saturated.kernel_discharge(
                &[FloatSemanticOperand::Meaning(FloatMeaning::from_f64(
                    f64::INFINITY
                ))],
                4096
            ),
            Err(FloatSemanticEvaluationError::IntegerConversion(
                FloatToIntegerError::MaterializationLimit {
                    required_bits: u64::MAX,
                    maximum_bits: 4096
                }
            ))
        );
        let u13 = FixedInteger::unsigned(13);
        let overflow = FloatMeaning::from_f64(8192.0);
        assert_eq!(
            FloatSemantics::to_integer_exact(&overflow, u13),
            Err(FloatToIntegerError::OutOfRange)
        );
        assert_eq!(
            FloatSemantics::to_integer_saturating(&overflow, u13, 13).unwrap(),
            BigInt::from_u64(8191)
        );
        assert_eq!(
            FloatSemantics::to_integer_saturating(&overflow, u13, 12),
            Err(FloatToIntegerError::MaterializationLimit {
                required_bits: 13,
                maximum_bits: 12
            })
        );
        assert_eq!(
            FloatSemantics::to_integer_saturating(
                &FloatMeaning::from_f64(f64::NEG_INFINITY),
                FixedInteger::signed(1),
                1
            )
            .unwrap(),
            BigInt::from_i64(-1)
        );
        assert_eq!(
            FloatSemantics::to_integer_saturating(
                &FloatMeaning::from_f64(f64::INFINITY),
                FixedInteger::signed(1),
                0
            )
            .unwrap(),
            BigInt::zero()
        );
    }

    use super::{
        FLOAT_SEMANTIC_OPERATIONS, FixedInteger, FloatClass, FloatFormat, FloatMeaning,
        FloatPolicyTrap, FloatSemanticComparison, FloatSemanticOperand, FloatSemanticOperation,
        FloatSemanticResult, FloatSemanticValueKind, FloatSemantics, FloatToIntegerError,
        RoundingDirection,
    };

    fn meaning32(value: f32) -> FloatMeaning {
        FloatMeaning::from_f32(value)
    }

    #[test]
    fn format_records_match_the_core_surface() {
        assert_eq!(FloatFormat::BINARY32.precision, 24);
        assert_eq!(FloatFormat::BINARY32.minimum_subnormal_exponent, -149);
        assert_eq!(FloatFormat::BINARY64.precision, 53);
        assert_eq!(FloatFormat::BINARY64.minimum_subnormal_exponent, -1074);
    }

    #[test]
    fn decode_encode_round_trips_non_nan_binary_values() {
        for bits in [
            0u32,
            1,
            0x007f_ffff,
            0x0080_0000,
            0x3f80_0000,
            0x7f7f_ffff,
            0x7f80_0000,
            0x8000_0000,
            0xff80_0000,
        ] {
            let value = f32::from_bits(bits);
            assert_eq!(FloatMeaning::from_f32(value).to_f32().to_bits(), bits);
        }
        for bits in [
            0u64,
            1,
            0x000f_ffff_ffff_ffff,
            0x0010_0000_0000_0000,
            0x3ff0_0000_0000_0000,
            0x7fef_ffff_ffff_ffff,
            0x7ff0_0000_0000_0000,
            0x8000_0000_0000_0000,
            0xfff0_0000_0000_0000,
        ] {
            let value = f64::from_bits(bits);
            assert_eq!(FloatMeaning::from_f64(value).to_f64().to_bits(), bits);
        }
    }

    #[test]
    fn binary32_arithmetic_rounds_at_each_named_operation() {
        let large = meaning32(16_777_216.0);
        let one = meaning32(1.0);
        let rounded_sum = FloatSemantics::add(FloatFormat::BINARY32, &large, &one);
        assert_eq!(rounded_sum.to_f32(), 16_777_216.0);
        let difference = FloatSemantics::subtract(FloatFormat::BINARY32, &rounded_sum, &large);
        assert_eq!(difference.to_f32().to_bits(), 0.0f32.to_bits());
    }

    #[test]
    fn specials_and_partial_comparisons_follow_the_settled_laws() {
        let positive_zero = meaning32(0.0);
        let negative_zero = meaning32(-0.0);
        let one = meaning32(1.0);
        let infinity = FloatSemantics::divide(FloatFormat::BINARY32, &one, &positive_zero);
        let negative_infinity = FloatSemantics::divide(FloatFormat::BINARY32, &one, &negative_zero);
        let nan = FloatSemantics::divide(FloatFormat::BINARY32, &positive_zero, &positive_zero);

        assert_eq!(infinity.to_f32(), f32::INFINITY);
        assert_eq!(negative_infinity.to_f32(), f32::NEG_INFINITY);
        assert!(nan.is_nan());
        assert!(!FloatSemantics::equal(&nan, &nan));
        assert!(FloatSemantics::not_equal(&nan, &nan));
        assert!(!FloatSemantics::less(&nan, &one));
        assert!(!FloatSemantics::greater_or_equal(&nan, &one));
        assert_eq!(
            FloatSemantics::minimum(&nan, &one).to_f32().to_bits(),
            one.to_f32().to_bits()
        );
        assert!(FloatSemantics::minimum(&one, &nan).is_nan());
    }

    #[test]
    fn signed_zero_survives_exact_arithmetic_and_rounding() {
        let negative_zero = meaning32(-0.0);
        let sum = FloatSemantics::add(FloatFormat::BINARY32, &negative_zero, &negative_zero);
        assert_eq!(sum.to_f32().to_bits(), (-0.0f32).to_bits());
        let negated = FloatSemantics::negate(FloatFormat::BINARY32, &negative_zero);
        assert_eq!(negated.to_f32().to_bits(), 0.0f32.to_bits());
    }

    #[test]
    fn fused_and_multiply_then_add_are_distinct_operations() {
        let left = meaning32(f32::from_bits(0x3f80_0001));
        let right = left.clone();
        let addend = meaning32(f32::from_bits(0xbf80_0002));
        let unfused =
            FloatSemantics::multiply_then_add(FloatFormat::BINARY32, &left, &right, &addend);
        let fused =
            FloatSemantics::fused_multiply_add(FloatFormat::BINARY32, &left, &right, &addend);

        assert_eq!(unfused.to_f32().to_bits(), 0.0f32.to_bits());
        assert_eq!(
            fused.to_f32().to_bits(),
            left.to_f32()
                .mul_add(right.to_f32(), addend.to_f32())
                .to_bits()
        );
        assert_ne!(fused.to_f32().to_bits(), unfused.to_f32().to_bits());
    }

    #[test]
    fn directed_rounding_is_explicit_and_sign_aware() {
        let positive_halfway = ExactFloat::from_decimal_str("1.000000059604644775390625").unwrap();
        let negative_halfway = ExactFloat::from_decimal_str("-1.000000059604644775390625").unwrap();

        let nearest = FloatSemantics::round_exact(FloatFormat::BINARY32, positive_halfway.clone());
        let upward = FloatSemantics::round_exact_directed(
            FloatFormat::BINARY32,
            positive_halfway,
            RoundingDirection::TowardPositive,
        );
        let downward = FloatSemantics::round_exact_directed(
            FloatFormat::BINARY32,
            negative_halfway,
            RoundingDirection::TowardNegative,
        );

        assert_eq!(nearest.to_f32().to_bits(), 1.0f32.to_bits());
        assert_eq!(upward.to_f32().to_bits(), 1.0f32.to_bits() + 1);
        assert_eq!(downward.to_f32().to_bits(), (-1.0f32).to_bits() + 1);
    }

    #[test]
    fn directed_named_arithmetic_rounds_the_exact_operation_result() {
        let one = meaning32(1.0);
        let half_ulp = meaning32(f32::from_bits((127 - 24) << 23));
        let nearest = FloatSemantics::add(FloatFormat::BINARY32, &one, &half_ulp);
        let upward = FloatSemantics::add_toward_positive(FloatFormat::BINARY32, &one, &half_ulp);
        assert_eq!(nearest.to_f32().to_bits(), 1.0f32.to_bits());
        assert_eq!(upward.to_f32().to_bits(), 1.0f32.to_bits() + 1);

        let three = meaning32(3.0);
        let quotient = FloatSemantics::divide_toward_zero(FloatFormat::BINARY32, &one, &three);
        assert_eq!(quotient.to_f32().to_bits(), 0x3eaa_aaaa);
    }

    #[test]
    fn directed_underflow_preserves_the_outward_minimum_subnormal() {
        let minimum_subnormal = ExactFloat::from_f32(f32::from_bits(1));
        let two = ExactFloat::from_decimal_str("2").unwrap();
        let positive_half = minimum_subnormal.div(&two);
        let negative_half = positive_half.negate();

        let nearest = FloatSemantics::round_exact(FloatFormat::BINARY32, positive_half.clone());
        let upward = FloatSemantics::round_exact_directed(
            FloatFormat::BINARY32,
            positive_half,
            RoundingDirection::TowardPositive,
        );
        let downward = FloatSemantics::round_exact_directed(
            FloatFormat::BINARY32,
            negative_half,
            RoundingDirection::TowardNegative,
        );

        assert_eq!(nearest.to_f32().to_bits(), 0.0f32.to_bits());
        assert_eq!(upward.to_f32().to_bits(), 1);
        assert_eq!(downward.to_f32().to_bits(), 0x8000_0001);
    }

    #[test]
    fn directed_overflow_chooses_infinity_or_max_finite_by_sign() {
        let positive = ExactFloat::from_decimal_str("1e1000").unwrap();
        let negative = ExactFloat::from_decimal_str("-1e1000").unwrap();

        let toward_zero = FloatSemantics::round_exact_directed(
            FloatFormat::BINARY32,
            positive.clone(),
            RoundingDirection::TowardZero,
        );
        let upward = FloatSemantics::round_exact_directed(
            FloatFormat::BINARY32,
            positive,
            RoundingDirection::TowardPositive,
        );
        let negative_upward = FloatSemantics::round_exact_directed(
            FloatFormat::BINARY32,
            negative,
            RoundingDirection::TowardPositive,
        );

        assert_eq!(toward_zero.to_f32().to_bits(), f32::MAX.to_bits());
        assert_eq!(upward.to_f32(), f32::INFINITY);
        assert_eq!(negative_upward.to_f32().to_bits(), (-f32::MAX).to_bits());
    }

    #[test]
    fn trapping_policy_checks_the_result_including_propagated_nonfinites() {
        let finite = meaning32(1.0);
        let nan = FloatMeaning::NaN;
        let infinity = FloatMeaning::Infinity { negative: false };

        assert_eq!(
            FloatSemantics::apply_trapping_policy(finite.clone()),
            Ok(finite)
        );
        assert_eq!(
            FloatSemantics::apply_trapping_policy(nan),
            Err(FloatPolicyTrap::NaNResult)
        );
        assert_eq!(
            FloatSemantics::apply_trapping_policy(infinity),
            Err(FloatPolicyTrap::InfinityResult)
        );
    }

    #[test]
    fn saturating_policy_clamps_only_finite_operand_magnitude_overflow() {
        let one = meaning32(1.0);
        let zero = meaning32(0.0);
        let infinity = FloatMeaning::Infinity { negative: false };
        let negative_infinity = FloatMeaning::Infinity { negative: true };
        let nan = FloatMeaning::NaN;

        assert_eq!(
            FloatSemantics::apply_saturating_policy(
                FloatFormat::BINARY32,
                &[&one, &one],
                infinity.clone(),
            )
            .to_f32()
            .to_bits(),
            f32::MAX.to_bits()
        );
        assert_eq!(
            FloatSemantics::apply_saturating_policy(
                FloatFormat::BINARY32,
                &[&one, &one],
                negative_infinity,
            )
            .to_f32()
            .to_bits(),
            (-f32::MAX).to_bits()
        );
        assert_eq!(
            FloatSemantics::apply_saturating_policy(
                FloatFormat::BINARY32,
                &[&infinity, &one],
                infinity.clone(),
            ),
            infinity
        );
        assert_eq!(
            FloatSemantics::apply_saturating_policy(
                FloatFormat::BINARY32,
                &[&one, &one],
                nan.clone()
            ),
            nan
        );
        assert_eq!(
            FloatSemantics::apply_saturating_policy(
                FloatFormat::BINARY32,
                &[&one, &one, &zero],
                FloatMeaning::Infinity { negative: false },
            )
            .to_f32()
            .to_bits(),
            f32::MAX.to_bits()
        );
        assert_eq!(
            FloatSemantics::apply_saturating_divide_policy(
                FloatFormat::BINARY32,
                &one,
                &zero,
                FloatMeaning::Infinity { negative: false },
            ),
            FloatMeaning::Infinity { negative: false }
        );
    }

    #[test]
    fn classification_distinguishes_normal_subnormal_zero_and_specials() {
        assert_eq!(
            FloatSemantics::classify(FloatFormat::BINARY32, &meaning32(f32::MIN_POSITIVE)),
            FloatClass::Normal { negative: false }
        );
        assert_eq!(
            FloatSemantics::classify(FloatFormat::BINARY32, &meaning32(f32::from_bits(1))),
            FloatClass::Subnormal { negative: false }
        );
        assert_eq!(
            FloatSemantics::classify(FloatFormat::BINARY32, &meaning32(-0.0)),
            FloatClass::Zero { negative: true }
        );
        assert_eq!(
            FloatSemantics::classify(FloatFormat::BINARY32, &meaning32(f32::NEG_INFINITY)),
            FloatClass::Infinity { negative: true }
        );
        assert_eq!(
            FloatSemantics::classify(FloatFormat::BINARY32, &meaning32(f32::NAN)),
            FloatClass::NaN
        );
    }

    #[test]
    fn float_to_integer_operations_share_truncation_and_policy_edges() {
        let negative_fraction = FloatMeaning::from_f64(-0.5);
        let negative_one = FloatMeaning::from_f64(-1.0);
        let positive_overflow = FloatMeaning::from_f64(256.0);
        let nan = FloatMeaning::from_f64(f64::NAN);

        assert_eq!(
            FloatSemantics::to_integer_exact(&negative_fraction, FixedInteger::unsigned(8))
                .unwrap()
                .to_u64(),
            Some(0)
        );
        assert_eq!(
            FloatSemantics::to_integer_exact(&negative_one, FixedInteger::unsigned(8)),
            Err(FloatToIntegerError::OutOfRange)
        );
        assert_eq!(
            FloatSemantics::to_integer_trapping(&nan, FixedInteger::signed(32)),
            Err(FloatToIntegerError::NonFinite)
        );
        assert_eq!(
            FloatSemantics::to_integer_saturating(
                &positive_overflow,
                FixedInteger::unsigned(8),
                4096
            )
            .unwrap()
            .to_u64(),
            Some(255)
        );
        assert_eq!(
            FloatSemantics::to_integer_saturating(&nan, FixedInteger::signed(32), 4096)
                .unwrap()
                .to_i64(),
            Some(0)
        );
    }

    #[test]
    fn integer_to_float_conversion_rounds_exact_full_width_values() {
        let maximum_u64 = BigInt::from_u64(u64::MAX);
        let nearest = FloatSemantics::from_integer(FloatFormat::BINARY64, &maximum_u64).to_f64();
        let downward =
            FloatSemantics::from_integer_toward_zero(FloatFormat::BINARY64, &maximum_u64).to_f64();

        assert_eq!(nearest.to_bits(), (u64::MAX as f64).to_bits());
        assert!(downward < nearest);
        assert_eq!(
            FloatSemantics::to_integer_saturating(
                &FloatMeaning::from_f64(f64::INFINITY),
                FixedInteger::unsigned(64),
                4096,
            )
            .unwrap()
            .to_u64(),
            Some(u64::MAX)
        );
    }

    #[test]
    fn square_root_is_exactly_rounded_without_host_arithmetic() {
        let two32 = meaning32(2.0);
        let nearest32 = FloatSemantics::square_root(FloatFormat::BINARY32, &two32);
        let upward32 = FloatSemantics::square_root_toward_positive(FloatFormat::BINARY32, &two32);
        assert_eq!(nearest32.to_f32().to_bits(), 0x3fb5_04f3);
        assert_eq!(upward32.to_f32().to_bits(), 0x3fb5_04f4);

        let two64 = FloatMeaning::from_f64(2.0);
        let nearest64 = FloatSemantics::square_root(FloatFormat::BINARY64, &two64);
        assert_eq!(nearest64.to_f64().to_bits(), 2.0f64.sqrt().to_bits());

        let perfect = FloatMeaning::from_f64(64.0);
        assert_eq!(
            FloatSemantics::square_root(FloatFormat::BINARY64, &perfect).to_f64(),
            8.0
        );
    }

    #[test]
    fn square_root_preserves_signed_zero_and_rejects_negative_values() {
        let negative_zero = meaning32(-0.0);
        let negative_one = meaning32(-1.0);
        let positive_infinity = meaning32(f32::INFINITY);

        assert_eq!(
            FloatSemantics::square_root(FloatFormat::BINARY32, &negative_zero)
                .to_f32()
                .to_bits(),
            (-0.0f32).to_bits()
        );
        assert!(FloatSemantics::square_root(FloatFormat::BINARY32, &negative_one).is_nan());
        assert_eq!(
            FloatSemantics::square_root(FloatFormat::BINARY32, &positive_infinity).to_f32(),
            f32::INFINITY
        );
    }

    #[test]
    fn square_root_matches_ieee_reference_across_format_boundaries() {
        for bits in [
            1u32,
            2,
            0x007f_ffff,
            0x0080_0000,
            0x3f00_0000,
            0x3f80_0000,
            0x4000_0000,
            0x4b00_0001,
            0x7f7f_ffff,
        ] {
            let value = f32::from_bits(bits);
            let actual =
                FloatSemantics::square_root(FloatFormat::BINARY32, &FloatMeaning::from_f32(value))
                    .to_f32();
            assert_eq!(
                actual.to_bits(),
                value.sqrt().to_bits(),
                "binary32 {bits:#x}"
            );
        }
        for bits in [
            1u64,
            0x000f_ffff_ffff_ffff,
            0x0010_0000_0000_0000,
            0x3fe0_0000_0000_0000,
            0x3ff0_0000_0000_0000,
            0x4000_0000_0000_0000,
            0x7fef_ffff_ffff_ffff,
        ] {
            let value = f64::from_bits(bits);
            let actual =
                FloatSemantics::square_root(FloatFormat::BINARY64, &FloatMeaning::from_f64(value))
                    .to_f64();
            assert_eq!(
                actual.to_bits(),
                value.sqrt().to_bits(),
                "binary64 {bits:#x}"
            );
        }
    }

    #[test]
    fn rows_are_distinct_by_complete_signature_and_overloads_do_not_collide() {
        assert_eq!(FLOAT_SEMANTIC_OPERATIONS.len(), 41);
        let keys = FLOAT_SEMANTIC_OPERATIONS
            .iter()
            .map(|row| (row.name, row.parameters(), row.result))
            .collect::<HashSet<_>>();
        assert_eq!(keys.len(), FLOAT_SEMANTIC_OPERATIONS.len());
        let commitments = FLOAT_SEMANTIC_OPERATIONS
            .iter()
            .map(|row| row.contract_identity().commitment)
            .collect::<HashSet<_>>();
        assert_eq!(commitments.len(), FLOAT_SEMANTIC_OPERATIONS.len());
        let i8_row = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "from_integer",
            &[
                FloatSemanticValueKind::Format,
                FloatSemanticValueKind::Integer(FixedInteger::signed(8)),
            ],
            FloatSemanticValueKind::Meaning,
        )
        .expect("i8 overload");
        let u64_row = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "from_integer",
            &[
                FloatSemanticValueKind::Format,
                FloatSemanticValueKind::Integer(FixedInteger::unsigned(64)),
            ],
            FloatSemanticValueKind::Meaning,
        )
        .expect("u64 overload");
        assert_ne!(i8_row.identity(), u64_row.identity());
        assert_ne!(i8_row.contract_identity(), u64_row.contract_identity());
    }

    #[test]
    fn leaf_spelling_alone_selects_nothing() {
        assert!(
            FloatSemanticOperation::from_source_identity(
                "Other",
                "add",
                &[
                    FloatSemanticValueKind::Format,
                    FloatSemanticValueKind::Meaning,
                    FloatSemanticValueKind::Meaning
                ],
                FloatSemanticValueKind::Meaning,
            )
            .is_none()
        );
        assert!(
            FloatSemanticOperation::from_source_identity(
                "FloatSemantics",
                "add",
                &[
                    FloatSemanticValueKind::Meaning,
                    FloatSemanticValueKind::Meaning
                ],
                FloatSemanticValueKind::Meaning,
            )
            .is_none(),
            "a drifted signature matches no row"
        );
        assert!(FloatSemanticOperation::names_a_row("FloatSemantics", "add"));
        assert!(!FloatSemanticOperation::names_a_row(
            "FloatSemantics",
            "hypot"
        ));
    }

    fn signature_operands(row: &FloatSemanticOperation) -> Vec<FloatSemanticOperand> {
        row.parameters()
            .iter()
            .map(|kind| match kind {
                FloatSemanticValueKind::Format => {
                    FloatSemanticOperand::Format(FloatFormat::BINARY32)
                }
                FloatSemanticValueKind::Meaning => {
                    FloatSemanticOperand::Meaning(FloatMeaning::from_f32(1.0))
                }
                FloatSemanticValueKind::Integer(_) => {
                    FloatSemanticOperand::Integer(BigInt::from_u64(1))
                }
                other => unreachable!("{other:?} is not a declared parameter kind"),
            })
            .collect()
    }

    fn row_named(
        name: &str,
        parameters: &[FloatSemanticValueKind],
        result: FloatSemanticValueKind,
    ) -> FloatSemanticOperation {
        FloatSemanticOperation::from_source_identity("FloatSemantics", name, parameters, result)
            .expect("catalog row")
    }

    #[test]
    fn contract_identities_rejoin_to_their_own_rows_only() {
        for row in FLOAT_SEMANTIC_OPERATIONS {
            let identity = row.contract_identity();
            let rejoined =
                FloatSemanticOperation::for_contract_identity(&identity).expect("rejoin");
            assert_eq!(
                rejoined.identity(),
                row.identity(),
                "{} rejoins to the declared row ordinal",
                row.name
            );
            assert_eq!(rejoined.contract_identity(), identity);
            let mut drifted = identity;
            drifted.commitment[0] ^= 1;
            assert!(FloatSemanticOperation::for_contract_identity(&drifted).is_none());
            let mut stale_version = identity;
            stale_version.catalog_version += 1;
            assert!(FloatSemanticOperation::for_contract_identity(&stale_version).is_none());
        }
        let mut out_of_range = FLOAT_SEMANTIC_OPERATIONS[0].contract_identity();
        out_of_range.operation = super::FloatSemanticOperationIdentity::Fixed(u8::MAX);
        assert!(FloatSemanticOperation::for_contract_identity(&out_of_range).is_none());
    }

    #[test]
    fn every_row_discharges_a_signature_shaped_application() {
        for row in FLOAT_SEMANTIC_OPERATIONS {
            let arguments = signature_operands(row);
            let discharge = row
                .kernel_discharge(&arguments, 4096)
                .unwrap_or_else(|error| panic!("{} failed to discharge: {error:?}", row.name));
            assert_eq!(discharge.contract, row.contract_identity());
            assert!(discharge.result.matches_kind(row.result));
        }
    }

    #[test]
    fn discharged_results_match_the_kernel_directly() {
        let left = FloatMeaning::from_f32(0.1);
        let right = FloatMeaning::from_f32(0.2);
        let add = row_named(
            "add",
            &[
                FloatSemanticValueKind::Format,
                FloatSemanticValueKind::Meaning,
                FloatSemanticValueKind::Meaning,
            ],
            FloatSemanticValueKind::Meaning,
        );
        let discharge = add
            .kernel_discharge(
                &[
                    FloatSemanticOperand::Format(FloatFormat::BINARY32),
                    FloatSemanticOperand::Meaning(left.clone()),
                    FloatSemanticOperand::Meaning(right.clone()),
                ],
                4096,
            )
            .expect("add discharges");
        assert_eq!(
            discharge.result,
            FloatSemanticResult::Meaning(FloatSemantics::add(FloatFormat::BINARY32, &left, &right))
        );
        let equal = row_named(
            "equal",
            &[
                FloatSemanticValueKind::Meaning,
                FloatSemanticValueKind::Meaning,
            ],
            FloatSemanticValueKind::Bool,
        );
        assert_eq!(
            equal
                .kernel_discharge(
                    &[
                        FloatSemanticOperand::Meaning(left.clone()),
                        FloatSemanticOperand::Meaning(right.clone()),
                    ],
                    4096
                )
                .expect("equal discharges")
                .result,
            FloatSemanticResult::Bool(FloatSemantics::equal(&left, &right))
        );
    }

    #[test]
    fn completed_comparison_laws_agree_with_ieee_kernels() {
        let comparisons = [
            FloatSemanticComparison::Equal,
            FloatSemanticComparison::NotEqual,
            FloatSemanticComparison::Less,
            FloatSemanticComparison::LessOrEqual,
            FloatSemanticComparison::Greater,
            FloatSemanticComparison::GreaterOrEqual,
        ];
        let values = [
            FloatMeaning::from_f32(0.0),
            FloatMeaning::from_f32(-0.0),
            FloatMeaning::from_f32(1.0),
            FloatMeaning::from_f32(-1.0),
            FloatMeaning::from_f32(f32::MIN_POSITIVE),
            FloatMeaning::from_f32(f32::MAX),
            FloatMeaning::from_f32(f32::INFINITY),
            FloatMeaning::from_f32(f32::NEG_INFINITY),
            FloatMeaning::NaN,
        ];
        for comparison in comparisons {
            for value in &values {
                assert_eq!(
                    comparison.same_meaning_truth(Some(FloatSemantics::is_nan(value))),
                    Some(comparison.evaluate(value, value))
                );
                assert_eq!(
                    comparison.unordered_truth(),
                    comparison.evaluate(value, &FloatMeaning::NaN)
                );
                assert_eq!(
                    comparison.unordered_truth(),
                    comparison.evaluate(&FloatMeaning::NaN, value)
                );
            }
        }
        assert_eq!(
            FloatSemanticComparison::Equal.same_meaning_truth(None),
            None
        );
        assert_eq!(
            FloatSemanticComparison::NotEqual.same_meaning_truth(None),
            None
        );
        assert_eq!(
            FloatSemanticComparison::Less.same_meaning_truth(None),
            Some(false)
        );
        assert_eq!(
            FloatSemanticComparison::Greater.same_meaning_truth(None),
            Some(false)
        );
    }

    #[test]
    fn discharge_keeps_meaning_equality_distinct_from_ieee_comparison() {
        let add = row_named(
            "add",
            &[
                FloatSemanticValueKind::Format,
                FloatSemanticValueKind::Meaning,
                FloatSemanticValueKind::Meaning,
            ],
            FloatSemanticValueKind::Meaning,
        );
        let discharged_nan = add
            .kernel_discharge(
                &[
                    FloatSemanticOperand::Format(FloatFormat::BINARY32),
                    FloatSemanticOperand::Meaning(FloatMeaning::NaN),
                    FloatSemanticOperand::Meaning(FloatMeaning::from_f32(1.0)),
                ],
                4096,
            )
            .expect("add(NaN, 1) discharges")
            .result;
        assert_eq!(
            discharged_nan,
            FloatSemanticResult::Meaning(FloatMeaning::NaN)
        );
        let equal = row_named(
            "equal",
            &[
                FloatSemanticValueKind::Meaning,
                FloatSemanticValueKind::Meaning,
            ],
            FloatSemanticValueKind::Bool,
        );
        assert_eq!(
            equal
                .kernel_discharge(
                    &[
                        FloatSemanticOperand::Meaning(FloatMeaning::NaN),
                        FloatSemanticOperand::Meaning(FloatMeaning::NaN),
                    ],
                    4096
                )
                .expect("equal(NaN, NaN) discharges")
                .result,
            FloatSemanticResult::Bool(false)
        );
    }

    #[test]
    fn discharge_preserves_the_signed_zero_distinction() {
        let add = row_named(
            "add",
            &[
                FloatSemanticValueKind::Format,
                FloatSemanticValueKind::Meaning,
                FloatSemanticValueKind::Meaning,
            ],
            FloatSemanticValueKind::Meaning,
        );
        let discharged = add
            .kernel_discharge(
                &[
                    FloatSemanticOperand::Format(FloatFormat::BINARY32),
                    FloatSemanticOperand::Meaning(FloatMeaning::from_f32(0.0)),
                    FloatSemanticOperand::Meaning(FloatMeaning::from_f32(-0.0)),
                ],
                4096,
            )
            .expect("add(+0, -0) discharges")
            .result;
        assert_eq!(
            discharged,
            FloatSemanticResult::Meaning(FloatMeaning::Zero { negative: false })
        );
        assert_ne!(
            discharged,
            FloatSemanticResult::Meaning(FloatMeaning::Zero { negative: true })
        );
        let equal = row_named(
            "equal",
            &[
                FloatSemanticValueKind::Meaning,
                FloatSemanticValueKind::Meaning,
            ],
            FloatSemanticValueKind::Bool,
        );
        assert_eq!(
            equal
                .kernel_discharge(
                    &[
                        FloatSemanticOperand::Meaning(FloatMeaning::from_f32(0.0)),
                        FloatSemanticOperand::Meaning(FloatMeaning::from_f32(-0.0)),
                    ],
                    4096
                )
                .expect("equal(+0, -0) discharges")
                .result,
            FloatSemanticResult::Bool(true)
        );
    }

    #[test]
    fn discharge_enforces_declared_carriers_and_conversion_domains() {
        let from_i8 = row_named(
            "from_integer",
            &[
                FloatSemanticValueKind::Format,
                FloatSemanticValueKind::Integer(FixedInteger::signed(8)),
            ],
            FloatSemanticValueKind::Meaning,
        );
        for value in [300_u64, 200_u64] {
            let literal = if value == 200_u64 {
                BigInt::from_u64(value).negate()
            } else {
                BigInt::from_u64(value)
            };
            assert!(
                from_i8
                    .kernel_discharge(
                        &[
                            FloatSemanticOperand::Format(FloatFormat::BINARY32),
                            FloatSemanticOperand::Integer(literal),
                        ],
                        4096
                    )
                    .is_err(),
                "out-of-carrier integer operand must not discharge"
            );
        }
        let discharged = from_i8
            .kernel_discharge(
                &[
                    FloatSemanticOperand::Format(FloatFormat::BINARY32),
                    FloatSemanticOperand::Integer(BigInt::from_u64(100)),
                ],
                4096,
            )
            .expect("in-carrier from_integer discharges")
            .result;
        assert_eq!(
            discharged,
            FloatSemanticResult::Meaning(FloatMeaning::from_f32(100.0))
        );

        let to_i8 = row_named(
            "to_i8",
            &[FloatSemanticValueKind::Meaning],
            FloatSemanticValueKind::Integer(FixedInteger::signed(8)),
        );
        assert!(
            to_i8
                .kernel_discharge(
                    &[FloatSemanticOperand::Meaning(FloatMeaning::from_f32(300.0))],
                    4096
                )
                .is_err()
        );
        let to_i8_saturating = row_named(
            "to_i8_saturating",
            &[FloatSemanticValueKind::Meaning],
            FloatSemanticValueKind::Integer(FixedInteger::signed(8)),
        );
        assert_eq!(
            to_i8_saturating
                .kernel_discharge(
                    &[FloatSemanticOperand::Meaning(FloatMeaning::from_f32(300.0))],
                    4096
                )
                .expect("saturating conversion discharges")
                .result,
            FloatSemanticResult::Integer(BigInt::from_u64(127))
        );
    }

    #[test]
    fn integer_carrier_images_preserve_rounded_bounds_and_signature() {
        for carrier in [
            FixedInteger::signed(8),
            FixedInteger::signed(16),
            FixedInteger::signed(32),
            FixedInteger::signed(64),
            FixedInteger::unsigned(8),
            FixedInteger::unsigned(16),
            FixedInteger::unsigned(32),
            FixedInteger::unsigned(64),
        ] {
            let conversion = row_named(
                "from_integer",
                &[
                    FloatSemanticValueKind::Format,
                    FloatSemanticValueKind::Integer(carrier),
                ],
                FloatSemanticValueKind::Meaning,
            );
            let minimum = FloatSemantics::integer_endpoint(carrier, true, 4096).unwrap();
            let maximum = FloatSemantics::integer_endpoint(carrier, false, 4096).unwrap();
            for format in [FloatFormat::BINARY32, FloatFormat::BINARY64] {
                let (lower, upper) = conversion
                    .integer_operand_image(format, carrier, &minimum, &maximum)
                    .expect("every builtin Integer image is finite");
                assert!(FloatSemantics::is_finite(&lower));
                assert!(FloatSemantics::is_finite(&upper));
                assert!(FloatSemantics::less_or_equal(&lower, &upper));
                if carrier == FixedInteger::unsigned(64) {
                    assert_eq!(upper, FloatMeaning::from_f64(18446744073709551616.0));
                }
            }
            assert!(
                conversion
                    .integer_operand_image(
                        FloatFormat::BINARY32,
                        carrier,
                        &minimum,
                        &maximum.add(&BigInt::from_u64(1))
                    )
                    .is_none()
            );
            assert!(
                conversion
                    .integer_operand_image(FloatFormat::BINARY32, carrier, &maximum, &minimum)
                    .is_none()
            );
            let other = if carrier == FixedInteger::signed(8) {
                FixedInteger::unsigned(8)
            } else {
                FixedInteger::signed(8)
            };
            assert!(
                conversion
                    .integer_operand_image(
                        FloatFormat::BINARY32,
                        other,
                        &BigInt::zero(),
                        &BigInt::from_u64(1)
                    )
                    .is_none()
            );
            assert!(
                conversion
                    .integer_operand_image(
                        FloatFormat {
                            precision: 8,
                            ..FloatFormat::BINARY32
                        },
                        carrier,
                        &minimum,
                        &maximum
                    )
                    .is_none()
            );
        }
    }

    #[test]
    fn ordered_predicate_images_follow_classification_laws() {
        let finite = row_named(
            "is_finite",
            &[FloatSemanticValueKind::Meaning],
            FloatSemanticValueKind::Bool,
        );
        let infinite = row_named(
            "is_infinite",
            &[FloatSemanticValueKind::Meaning],
            FloatSemanticValueKind::Bool,
        );
        let nan = row_named(
            "is_nan",
            &[FloatSemanticValueKind::Meaning],
            FloatSemanticValueKind::Bool,
        );
        let lower = FloatMeaning::from_f64(-1.0);
        let upper = FloatMeaning::from_f64(1.0);
        assert_eq!(
            finite.ordered_interval_predicate(&lower, &upper),
            Some(true)
        );
        assert_eq!(
            infinite.ordered_interval_predicate(&lower, &upper),
            Some(false)
        );
        assert_eq!(nan.ordered_interval_predicate(&lower, &upper), Some(false));
        let negative_infinity = FloatMeaning::from_f64(f64::NEG_INFINITY);
        let positive_infinity = FloatMeaning::from_f64(f64::INFINITY);
        assert_eq!(
            finite.ordered_interval_predicate(&negative_infinity, &positive_infinity),
            None
        );
        assert_eq!(
            infinite.ordered_interval_predicate(&negative_infinity, &positive_infinity),
            None
        );
        assert_eq!(
            infinite.ordered_interval_predicate(&positive_infinity, &positive_infinity),
            Some(true)
        );
        assert_eq!(finite.ordered_interval_predicate(&upper, &lower), None);
        for predicate in [finite, infinite, nan] {
            assert_eq!(
                predicate.ordered_interval_predicate(&FloatMeaning::NaN, &FloatMeaning::NaN),
                None
            );
        }
    }

    #[test]
    fn malformed_applications_do_not_discharge() {
        let add = row_named(
            "add",
            &[
                FloatSemanticValueKind::Format,
                FloatSemanticValueKind::Meaning,
                FloatSemanticValueKind::Meaning,
            ],
            FloatSemanticValueKind::Meaning,
        );
        let one = FloatMeaning::from_f32(1.0);
        assert!(
            add.kernel_discharge(
                &[
                    FloatSemanticOperand::Format(FloatFormat::BINARY32),
                    FloatSemanticOperand::Meaning(one.clone()),
                ],
                4096
            )
            .is_err()
        );
        assert!(
            add.kernel_discharge(
                &[
                    FloatSemanticOperand::Meaning(one.clone()),
                    FloatSemanticOperand::Meaning(one.clone()),
                    FloatSemanticOperand::Meaning(one),
                ],
                4096
            )
            .is_err(),
            "a Meaning cannot stand at the declared Format position"
        );
        let other_format = FloatFormat {
            precision: 8,
            ..FloatFormat::BINARY32
        };
        assert!(
            add.kernel_discharge(
                &[
                    FloatSemanticOperand::Format(other_format),
                    FloatSemanticOperand::Meaning(FloatMeaning::from_f32(1.0)),
                    FloatSemanticOperand::Meaning(FloatMeaning::from_f32(2.0)),
                ],
                4096
            )
            .is_err()
        );
    }
}
