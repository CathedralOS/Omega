//! Numeric operations retained by Psi, their checked signatures and canonical wire identities.
use crate::arithmetic::ArithmeticDomain;
use crate::integer_carrier::{FixedInteger, IntegerSign};
use language_core::BuiltinFunction;

mod core_integer_operators;
pub use core_integer_operators::{
    CoreIntegerOperand, CoreIntegerOperatorSignature, CoreIntegerResult, ScalarBinaryOperation,
};

macro_rules! tagged_vocabulary {
    ($documentation:literal, $tag_documentation:literal, $name:ident, $tag_fn:ident { $($variant:ident => $spelling:literal = $tag:literal),+ $(,)? }) => {
        #[doc = $documentation]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($variant,)+
        }

        impl $name {
            pub const ALL: [Self; [$(Self::$variant),+].len()] = [$(Self::$variant),+];

            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $spelling,)+
                }
            }
        }

        #[doc = $tag_documentation]
        pub const fn $tag_fn(value: $name) -> u8 {
            match value {
                $($name::$variant => $tag,)+
            }
        }
    };
}

/// Numeric signatures retain the integer's complete meaning independently of realization limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NumericType {
    Integer(FixedInteger),
    F32,
    F64,
}

impl NumericType {
    pub const ENCODED_SIZE: usize = 10;

    pub const fn is_float(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }

    /// Fixed-size metadata preserves all widths without constructing their values or bounds.
    pub fn canonical_encoding(self) -> [u8; Self::ENCODED_SIZE] {
        let mut bytes = [0; Self::ENCODED_SIZE];
        match self {
            Self::Integer(carrier) => {
                bytes[1] = u8::from(carrier.is_signed());
                bytes[2..].copy_from_slice(&carrier.width().to_le_bytes());
            }
            Self::F32 => bytes[0] = 1,
            Self::F64 => bytes[0] = 2,
        }
        bytes
    }

    pub fn from_canonical_encoding(bytes: [u8; Self::ENCODED_SIZE]) -> Option<Self> {
        let value = match bytes[0] {
            0 => {
                let sign = match bytes[1] {
                    0 => IntegerSign::Unsigned,
                    1 => IntegerSign::Signed,
                    _ => return None,
                };
                Self::Integer(FixedInteger::new(
                    sign,
                    u64::from_le_bytes(bytes[2..].try_into().ok()?),
                )?)
            }
            1 => Self::F32,
            2 => Self::F64,
            _ => return None,
        };
        (value.canonical_encoding() == bytes).then_some(value)
    }
}

tagged_vocabulary!(
    "Closed primitive-float binary operation vocabulary: declaration order is the canonical encoded order.",
    "Canonical tag of a primitive float binary operation.",
    PrimitiveFloatBinaryOperation,
    primitive_float_operation_tag
    {
        Add => "add" = 0,
        Subtract => "subtract" = 1,
        Multiply => "multiply" = 2,
        Divide => "divide" = 3,
        Equal => "equal" = 4,
        NotEqual => "not_equal" = 5,
        Less => "less" = 6,
        LessOrEqual => "less_or_equal" = 7,
        Greater => "greater" = 8,
        GreaterOrEqual => "greater_or_equal" = 9,
    }
);

tagged_vocabulary!(
    "The authored integer comparison spellings, before any emission mapping. `>`/`>=`/`!=` keep their authored identity here even though Terminal emission normalizes them onto the swapped/negated three-operation roster; the recorded emitted shape is what the proposal replays, while this value is what the selected provider plan commits to.",
    "Wire tag for a primitive integer-comparison operation — the six-row encoding every authority table and its checker spells identically.",
    PrimitiveIntegerComparisonOperation,
    primitive_integer_comparison_operation_tag
    {
        Equal => "equal" = 0,
        NotEqual => "not_equal" = 1,
        Less => "less" = 2,
        LessOrEqual => "less_or_equal" = 3,
        Greater => "greater" = 4,
        GreaterOrEqual => "greater_or_equal" = 5,
    }
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NumericIntrinsic {
    Builtin {
        function: BuiltinFunction,
        format: FloatFormat,
    },
    FloatBinary {
        operation: PrimitiveFloatBinaryOperation,
        format: FloatFormat,
    },
    IntegerComparison {
        operation: PrimitiveIntegerComparisonOperation,
        integer_type: NumericType,
    },
    Negation(FloatFormat),
    Conversion {
        source: NumericType,
        target: NumericType,
        domain: ArithmeticDomain,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericResult {
    Primitive(NumericType),
    Boolean,
    FloatClass,
}

#[derive(Clone, Copy)]
enum FloatResult {
    Float,
    Boolean,
    Class,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
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
            Self::F32 => Some(bignum::float_bits::binary32_from_bits(
                u32::try_from(bits).ok()?,
            )),
            Self::F64 => Some(f64::from_bits(bits)),
        }
    }

    pub fn capture_wide_representation(self, value: f64) -> Option<u64> {
        match self {
            Self::F32 => {
                let bits = bignum::float_bits::binary32_to_bits(value);
                (bignum::float_bits::binary32_from_bits(bits).to_bits() == value.to_bits())
                    .then_some(u64::from(bits))
            }
            Self::F64 => Some(value.to_bits()),
        }
    }
}

/// Each row binds one operation to its accepted binary32 and binary64 execution identities.
/// Each row retains its published wire tag.
#[derive(Clone, Copy)]
pub struct FloatBuiltin {
    tag: u8,
    arity: usize,
    result: FloatResult,
    binary32: BuiltinFunction,
    binary64: BuiltinFunction,
}

impl FloatBuiltin {
    pub fn intrinsic(self, format: FloatFormat) -> NumericIntrinsic {
        NumericIntrinsic::Builtin {
            format,
            function: match format {
                FloatFormat::F32 => self.binary32,
                FloatFormat::F64 => self.binary64,
            },
        }
    }
}

macro_rules! float_builtins {
    ($( $tag:literal => $name:ident($arity:literal, $result:ident, $binary32:ident, $binary64:ident) ),+ $(,)?) => {
        impl FloatBuiltin {
            $(pub const $name: Self = Self {
                tag: $tag,
                arity: $arity,
                result: FloatResult::$result,
                binary32: BuiltinFunction::$binary32,
                binary64: BuiltinFunction::$binary64,
            };)+
            const ALL: &'static [Self] = &[$(Self::$name,)+];
        }
    };
}

float_builtins! {
    0 => MINIMUM(2, Float, FloatMinimum, FloatMinimum),
    1 => MAXIMUM(2, Float, FloatMaximum, FloatMaximum),
    2 => SQUARE_ROOT(1, Float, FloatSquareRoot, FloatSquareRoot),
    3 => MULTIPLY_THEN_ADD(3, Float, FloatMultiplyThenAddF32, FloatMultiplyThenAddF64),
    4 => FUSED_MULTIPLY_ADD(3, Float, FloatFusedMultiplyAddF32, FloatFusedMultiplyAddF64),
    5 => IS_NAN(1, Boolean, FloatIsNan, FloatIsNan),
    6 => IS_FINITE(1, Boolean, FloatIsFinite, FloatIsFinite),
    7 => IS_INFINITE(1, Boolean, FloatIsInfinite, FloatIsInfinite),
    8 => IS_NORMAL(1, Boolean, FloatIsNormal, FloatIsNormal),
    9 => IS_SUBNORMAL(1, Boolean, FloatIsSubnormal, FloatIsSubnormal),
    10 => CLASSIFY(1, Class, FloatClassifyF32, FloatClassifyF64),
    11 => ADD_TOWARD_ZERO(2, Float, FloatAddTowardZeroF32, FloatAddTowardZeroF64),
    12 => ADD_TOWARD_POSITIVE(2, Float, FloatAddTowardPositiveF32, FloatAddTowardPositiveF64),
    13 => ADD_TOWARD_NEGATIVE(2, Float, FloatAddTowardNegativeF32, FloatAddTowardNegativeF64),
    14 => SUBTRACT_TOWARD_ZERO(2, Float, FloatSubtractTowardZeroF32, FloatSubtractTowardZeroF64),
    15 => SUBTRACT_TOWARD_POSITIVE(2, Float, FloatSubtractTowardPositiveF32, FloatSubtractTowardPositiveF64),
    16 => SUBTRACT_TOWARD_NEGATIVE(2, Float, FloatSubtractTowardNegativeF32, FloatSubtractTowardNegativeF64),
    17 => MULTIPLY_TOWARD_ZERO(2, Float, FloatMultiplyTowardZeroF32, FloatMultiplyTowardZeroF64),
    18 => MULTIPLY_TOWARD_POSITIVE(2, Float, FloatMultiplyTowardPositiveF32, FloatMultiplyTowardPositiveF64),
    19 => MULTIPLY_TOWARD_NEGATIVE(2, Float, FloatMultiplyTowardNegativeF32, FloatMultiplyTowardNegativeF64),
    20 => DIVIDE_TOWARD_ZERO(2, Float, FloatDivideTowardZeroF32, FloatDivideTowardZeroF64),
    21 => DIVIDE_TOWARD_POSITIVE(2, Float, FloatDivideTowardPositiveF32, FloatDivideTowardPositiveF64),
    22 => DIVIDE_TOWARD_NEGATIVE(2, Float, FloatDivideTowardNegativeF32, FloatDivideTowardNegativeF64),
    23 => SQUARE_ROOT_TOWARD_ZERO(1, Float, FloatSqrtTowardZeroF32, FloatSqrtTowardZeroF64),
    24 => SQUARE_ROOT_TOWARD_POSITIVE(1, Float, FloatSqrtTowardPositiveF32, FloatSqrtTowardPositiveF64),
    25 => SQUARE_ROOT_TOWARD_NEGATIVE(1, Float, FloatSqrtTowardNegativeF32, FloatSqrtTowardNegativeF64),
    26 => FUSED_MULTIPLY_ADD_TOWARD_ZERO(3, Float, FloatFusedMultiplyAddTowardZeroF32, FloatFusedMultiplyAddTowardZeroF64),
    27 => FUSED_MULTIPLY_ADD_TOWARD_POSITIVE(3, Float, FloatFusedMultiplyAddTowardPositiveF32, FloatFusedMultiplyAddTowardPositiveF64),
    28 => FUSED_MULTIPLY_ADD_TOWARD_NEGATIVE(3, Float, FloatFusedMultiplyAddTowardNegativeF32, FloatFusedMultiplyAddTowardNegativeF64),
}

pub struct NumericSignature {
    pub operand: NumericType,
    pub operand_count: usize,
    pub result: NumericResult,
    pub execution: NumericIntrinsic,
}

impl NumericSignature {
    pub fn accepts(
        &self,
        parameters: impl ExactSizeIterator<Item = Option<NumericType>>,
        result: Option<NumericResult>,
    ) -> bool {
        parameters.len() == self.operand_count
            && parameters
                .into_iter()
                .all(|parameter| parameter == Some(self.operand))
            && result == Some(self.result)
    }
}

impl NumericIntrinsic {
    pub fn signature(self) -> Option<NumericSignature> {
        let float_type = |format| match format {
            FloatFormat::F32 => NumericType::F32,
            FloatFormat::F64 => NumericType::F64,
        };
        let (operand, operand_count, result) = match self {
            Self::Builtin { format, .. } => {
                let row = FloatBuiltin::ALL
                    .iter()
                    .find(|row| row.intrinsic(format) == self)?;
                let operand = float_type(format);
                (
                    operand,
                    row.arity,
                    match row.result {
                        FloatResult::Float => NumericResult::Primitive(operand),
                        FloatResult::Boolean => NumericResult::Boolean,
                        FloatResult::Class => NumericResult::FloatClass,
                    },
                )
            }
            Self::FloatBinary { operation, format } => {
                let operand = float_type(format);
                let result = if matches!(
                    operation,
                    PrimitiveFloatBinaryOperation::Add
                        | PrimitiveFloatBinaryOperation::Subtract
                        | PrimitiveFloatBinaryOperation::Multiply
                        | PrimitiveFloatBinaryOperation::Divide
                ) {
                    NumericResult::Primitive(operand)
                } else {
                    NumericResult::Boolean
                };
                (operand, 2, result)
            }
            Self::IntegerComparison { integer_type, .. } => {
                if integer_type.is_float() {
                    return None;
                }
                (integer_type, 2, NumericResult::Boolean)
            }
            Self::Negation(format) => {
                let operand = float_type(format);
                (operand, 1, NumericResult::Primitive(operand))
            }
            Self::Conversion {
                source,
                target,
                domain,
            } => {
                if source == target
                    || (!source.is_float() && !target.is_float())
                    || (target.is_float() && domain != ArithmeticDomain::Exact)
                    || domain == ArithmeticDomain::Wrapping
                {
                    return None;
                }
                (source, 1, NumericResult::Primitive(target))
            }
        };
        Some(NumericSignature {
            operand,
            operand_count,
            result,
            execution: self,
        })
    }

    pub const ENCODED_SIZE: usize = 4 + 2 * NumericType::ENCODED_SIZE;
    const SECOND_TYPE_OFFSET: usize = 4 + NumericType::ENCODED_SIZE;

    pub fn canonical_encoding(self) -> Option<[u8; Self::ENCODED_SIZE]> {
        self.signature()?;
        let format_tag = |format| match format {
            FloatFormat::F32 => 0,
            FloatFormat::F64 => 1,
        };
        let mut bytes = [0; Self::ENCODED_SIZE];
        let header = match self {
            Self::Builtin { format, .. } => {
                let row = FloatBuiltin::ALL
                    .iter()
                    .find(|row| row.intrinsic(format) == self)?;
                [0, row.tag, format_tag(format), 0]
            }
            Self::FloatBinary { operation, format } => [
                1,
                primitive_float_operation_tag(operation),
                format_tag(format),
                0,
            ],
            Self::IntegerComparison {
                operation,
                integer_type,
            } => {
                bytes[4..Self::SECOND_TYPE_OFFSET]
                    .copy_from_slice(&integer_type.canonical_encoding());
                [
                    2,
                    primitive_integer_comparison_operation_tag(operation),
                    0,
                    0,
                ]
            }
            Self::Negation(format) => [3, format_tag(format), 0, 0],
            Self::Conversion {
                source,
                target,
                domain,
            } => {
                bytes[4..Self::SECOND_TYPE_OFFSET].copy_from_slice(&source.canonical_encoding());
                bytes[Self::SECOND_TYPE_OFFSET..].copy_from_slice(&target.canonical_encoding());
                [
                    4,
                    match domain {
                        ArithmeticDomain::Exact => 0,
                        ArithmeticDomain::Wrapping => 1,
                        ArithmeticDomain::Saturating => 2,
                        ArithmeticDomain::Trapping => 3,
                    },
                    0,
                    0,
                ]
            }
        };
        bytes[..4].copy_from_slice(&header);
        Some(bytes)
    }

    pub fn from_canonical_encoding(bytes: [u8; Self::ENCODED_SIZE]) -> Option<Self> {
        let [kind, first, second, _]: [u8; 4] = bytes[..4].try_into().ok()?;
        let format = |tag| match tag {
            0 => Some(FloatFormat::F32),
            1 => Some(FloatFormat::F64),
            _ => None,
        };
        let numeric = |offset| {
            NumericType::from_canonical_encoding(
                bytes[offset..offset + NumericType::ENCODED_SIZE]
                    .try_into()
                    .ok()?,
            )
        };
        let meaning = match kind {
            0 => FloatBuiltin::ALL
                .get(usize::from(first))?
                .intrinsic(format(second)?),
            1 => Self::FloatBinary {
                operation: *PrimitiveFloatBinaryOperation::ALL.get(usize::from(first))?,
                format: format(second)?,
            },
            2 => Self::IntegerComparison {
                operation: *PrimitiveIntegerComparisonOperation::ALL.get(usize::from(first))?,
                integer_type: numeric(4)?,
            },
            3 => Self::Negation(format(first)?),
            4 => Self::Conversion {
                source: numeric(4)?,
                target: numeric(Self::SECOND_TYPE_OFFSET)?,
                domain: match first {
                    0 => ArithmeticDomain::Exact,
                    1 => ArithmeticDomain::Wrapping,
                    2 => ArithmeticDomain::Saturating,
                    3 => ArithmeticDomain::Trapping,
                    _ => return None,
                },
            },
            _ => return None,
        };
        (meaning.canonical_encoding()? == bytes).then_some(meaning)
    }

    pub fn builtin_format(self) -> Option<FloatFormat> {
        match self {
            Self::Builtin { format, .. } => Some(format),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_width_encoding_preserves_sign_and_refuses_malformed_types() {
        for width in [1, 13, 128, 257, u64::MAX] {
            for integer in [FixedInteger::signed(width), FixedInteger::unsigned(width)] {
                let value = NumericType::Integer(integer);
                let bytes = value.canonical_encoding();
                assert_eq!(NumericType::from_canonical_encoding(bytes), Some(value));
                let meaning = NumericIntrinsic::IntegerComparison {
                    operation: PrimitiveIntegerComparisonOperation::Less,
                    integer_type: value,
                };
                let encoded = meaning.canonical_encoding().unwrap();
                assert_eq!(
                    NumericIntrinsic::from_canonical_encoding(encoded),
                    Some(meaning)
                );
                let mut invalid = bytes;
                invalid[1] = 2;
                assert_eq!(NumericType::from_canonical_encoding(invalid), None);
                invalid = bytes;
                invalid[2..].fill(0);
                assert_eq!(NumericType::from_canonical_encoding(invalid), None);
            }
        }
        let mut padded_float = NumericType::F32.canonical_encoding();
        padded_float[9] = 1;
        assert_eq!(NumericType::from_canonical_encoding(padded_float), None);
    }

    #[test]
    fn float_builtins_keep_their_wire_rows_and_reject_noncanonical_bytes() {
        for (row, builtin) in FloatBuiltin::ALL.iter().enumerate() {
            for (tag, format) in [(0, FloatFormat::F32), (1, FloatFormat::F64)] {
                let intrinsic = builtin.intrinsic(format);
                let mut bytes = [0; NumericIntrinsic::ENCODED_SIZE];
                bytes[..4].copy_from_slice(&[0, row as u8, tag, 0]);
                assert_eq!(intrinsic.canonical_encoding(), Some(bytes));
                assert_eq!(
                    NumericIntrinsic::from_canonical_encoding(bytes),
                    Some(intrinsic)
                );
                bytes[3] = 1;
                assert_eq!(NumericIntrinsic::from_canonical_encoding(bytes), None);
            }
        }
        assert_eq!(
            FloatBuiltin::IS_NAN
                .intrinsic(FloatFormat::F64)
                .canonical_encoding()
                .unwrap()[..4],
            [0, 5, 1, 0]
        );
    }
}
