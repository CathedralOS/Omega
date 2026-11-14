/*
We define the fixed-width integer and IEEE-754 floating-point intrinsic
catalog, operation signatures, and overflow/rounding semantics.
*/

use numerics::arithmetic::ArithmeticDomain;
use numerics::integer_carrier::{FixedInteger, IntegerSign};
use numerics::literals::FloatFormat;
use symbols::BuiltinFunction;

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

macro_rules! float_op {
    ($name:literal => $arity:literal => $result:ident => $narrow:ident => $wide:ident) => {
        (
            $name,
            $arity,
            FloatResult::$result,
            BuiltinFunction::$narrow,
            BuiltinFunction::$wide,
        )
    };
}

const FLOAT_OPERATIONS: &[(&str, usize, FloatResult, BuiltinFunction, BuiltinFunction)] = &[
    float_op!("minimum" => 2 => Float => FloatMinimum => FloatMinimum),
    float_op!("maximum" => 2 => Float => FloatMaximum => FloatMaximum),
    float_op!("square_root" => 1 => Float => FloatSquareRoot => FloatSquareRoot),
    (
        "multiply_then_add",
        3,
        FloatResult::Float,
        BuiltinFunction::FloatMultiplyThenAddF32,
        BuiltinFunction::FloatMultiplyThenAddF64,
    ),
    (
        "fused_multiply_add",
        3,
        FloatResult::Float,
        BuiltinFunction::FloatFusedMultiplyAddF32,
        BuiltinFunction::FloatFusedMultiplyAddF64,
    ),
    float_op!("is_nan" => 1 => Boolean => FloatIsNan => FloatIsNan),
    float_op!("is_finite" => 1 => Boolean => FloatIsFinite => FloatIsFinite),
    float_op!("is_infinite" => 1 => Boolean => FloatIsInfinite => FloatIsInfinite),
    float_op!("is_normal" => 1 => Boolean => FloatIsNormal => FloatIsNormal),
    float_op!("is_subnormal" => 1 => Boolean => FloatIsSubnormal => FloatIsSubnormal),
    float_op!("classify" => 1 => Class => FloatClassifyF32 => FloatClassifyF64),
    float_op!("add_toward_zero" => 2 => Float => FloatAddTowardZeroF32 => FloatAddTowardZeroF64),
    (
        "add_toward_positive",
        2,
        FloatResult::Float,
        BuiltinFunction::FloatAddTowardPositiveF32,
        BuiltinFunction::FloatAddTowardPositiveF64,
    ),
    (
        "add_toward_negative",
        2,
        FloatResult::Float,
        BuiltinFunction::FloatAddTowardNegativeF32,
        BuiltinFunction::FloatAddTowardNegativeF64,
    ),
    (
        "subtract_toward_zero",
        2,
        FloatResult::Float,
        BuiltinFunction::FloatSubtractTowardZeroF32,
        BuiltinFunction::FloatSubtractTowardZeroF64,
    ),
    (
        "subtract_toward_positive",
        2,
        FloatResult::Float,
        BuiltinFunction::FloatSubtractTowardPositiveF32,
        BuiltinFunction::FloatSubtractTowardPositiveF64,
    ),
    (
        "subtract_toward_negative",
        2,
        FloatResult::Float,
        BuiltinFunction::FloatSubtractTowardNegativeF32,
        BuiltinFunction::FloatSubtractTowardNegativeF64,
    ),
    (
        "multiply_toward_zero",
        2,
        FloatResult::Float,
        BuiltinFunction::FloatMultiplyTowardZeroF32,
        BuiltinFunction::FloatMultiplyTowardZeroF64,
    ),
    (
        "multiply_toward_positive",
        2,
        FloatResult::Float,
        BuiltinFunction::FloatMultiplyTowardPositiveF32,
        BuiltinFunction::FloatMultiplyTowardPositiveF64,
    ),
    (
        "multiply_toward_negative",
        2,
        FloatResult::Float,
        BuiltinFunction::FloatMultiplyTowardNegativeF32,
        BuiltinFunction::FloatMultiplyTowardNegativeF64,
    ),
    (
        "divide_toward_zero",
        2,
        FloatResult::Float,
        BuiltinFunction::FloatDivideTowardZeroF32,
        BuiltinFunction::FloatDivideTowardZeroF64,
    ),
    (
        "divide_toward_positive",
        2,
        FloatResult::Float,
        BuiltinFunction::FloatDivideTowardPositiveF32,
        BuiltinFunction::FloatDivideTowardPositiveF64,
    ),
    (
        "divide_toward_negative",
        2,
        FloatResult::Float,
        BuiltinFunction::FloatDivideTowardNegativeF32,
        BuiltinFunction::FloatDivideTowardNegativeF64,
    ),
    (
        "square_root_toward_zero",
        1,
        FloatResult::Float,
        BuiltinFunction::FloatSqrtTowardZeroF32,
        BuiltinFunction::FloatSqrtTowardZeroF64,
    ),
    (
        "square_root_toward_positive",
        1,
        FloatResult::Float,
        BuiltinFunction::FloatSqrtTowardPositiveF32,
        BuiltinFunction::FloatSqrtTowardPositiveF64,
    ),
    (
        "square_root_toward_negative",
        1,
        FloatResult::Float,
        BuiltinFunction::FloatSqrtTowardNegativeF32,
        BuiltinFunction::FloatSqrtTowardNegativeF64,
    ),
    (
        "fused_multiply_add_toward_zero",
        3,
        FloatResult::Float,
        BuiltinFunction::FloatFusedMultiplyAddTowardZeroF32,
        BuiltinFunction::FloatFusedMultiplyAddTowardZeroF64,
    ),
    (
        "fused_multiply_add_toward_positive",
        3,
        FloatResult::Float,
        BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF32,
        BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF64,
    ),
    (
        "fused_multiply_add_toward_negative",
        3,
        FloatResult::Float,
        BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF32,
        BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF64,
    ),
];

pub struct NamedNumericIntrinsic {
    pub operand: NumericType,
    pub operand_count: usize,
    pub result: NumericResult,
    pub execution: NumericIntrinsic,
}

impl NamedNumericIntrinsic {
    pub fn resolve(namespace: &str, name: &str, domain: ArithmeticDomain) -> Option<Self> {
        let target = numeric_primitive(namespace)?;
        let meaning = if let Some(source_name) = name.strip_prefix("from_") {
            let source = conversion_operand(source_name)?;
            NumericIntrinsic::Conversion {
                source,
                target,
                domain: if target.is_float() {
                    ArithmeticDomain::Exact
                } else {
                    domain
                },
            }
        } else {
            let format = match target {
                NumericType::F32 => FloatFormat::F32,
                NumericType::F64 => FloatFormat::F64,
                _ => return None,
            };
            if name == "negate" {
                NumericIntrinsic::Negation(format)
            } else {
                let (_, _, _, f32_function, f64_function) =
                    FLOAT_OPERATIONS.iter().find(|row| row.0 == name)?;
                NumericIntrinsic::Builtin {
                    function: match format {
                        FloatFormat::F32 => *f32_function,
                        FloatFormat::F64 => *f64_function,
                    },
                    format,
                }
            }
        };
        meaning.signature()
    }

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
    pub fn signature(self) -> Option<NamedNumericIntrinsic> {
        let float_type = |format| match format {
            FloatFormat::F32 => NumericType::F32,
            FloatFormat::F64 => NumericType::F64,
        };
        let (operand, operand_count, result) = match self {
            Self::Builtin { function, format } => {
                let (_, count, result, _, _) =
                    FLOAT_OPERATIONS
                        .iter()
                        .find(|(_, _, _, f32_function, f64_function)| {
                            function
                                == match format {
                                    FloatFormat::F32 => *f32_function,
                                    FloatFormat::F64 => *f64_function,
                                }
                        })?;
                let operand = float_type(format);
                (
                    operand,
                    *count,
                    match result {
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
        Some(NamedNumericIntrinsic {
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
            Self::Builtin { function, format } => {
                let row =
                    FLOAT_OPERATIONS
                        .iter()
                        .position(|(_, _, _, f32_function, f64_function)| {
                            function
                                == match format {
                                    FloatFormat::F32 => *f32_function,
                                    FloatFormat::F64 => *f64_function,
                                }
                        })?;
                [0, u8::try_from(row).ok()?, format_tag(format), 0]
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
            0 => {
                let (_, _, _, f32_function, f64_function) =
                    FLOAT_OPERATIONS.get(usize::from(first))?;
                let format = format(second)?;
                Self::Builtin {
                    function: match format {
                        FloatFormat::F32 => *f32_function,
                        FloatFormat::F64 => *f64_function,
                    },
                    format,
                }
            }
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

    pub fn builtin_format(self) -> Option<semantic_vocabulary::IeeeFloatFormat> {
        match self {
            Self::Builtin { format, .. } => Some(match format {
                FloatFormat::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
                FloatFormat::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
            }),
            _ => None,
        }
    }
}

impl NumericType {
    pub fn scalar_type(self) -> Option<semantic_vocabulary::ScalarType> {
        use semantic_vocabulary::{IeeeFloatFormat, IntegerType, ScalarType};
        match self {
            Self::Integer(carrier) => IntegerType::from_fixed(carrier).map(ScalarType::Integer),
            Self::F32 => Some(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)),
            Self::F64 => Some(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)),
        }
    }
}

fn numeric_primitive(namespace: &str) -> Option<NumericType> {
    match namespace {
        "F32" => return Some(NumericType::F32),
        "F64" => return Some(NumericType::F64),
        _ => {}
    }
    let (sign, digits) = if let Some(digits) = namespace.strip_prefix('I') {
        (IntegerSign::Signed, digits)
    } else {
        (IntegerSign::Unsigned, namespace.strip_prefix('U')?)
    };
    FixedInteger::from_decimal_digits(sign, digits).map(NumericType::Integer)
}

fn conversion_operand(name: &str) -> Option<NumericType> {
    Some(match name {
        "f32" => NumericType::F32,
        "f64" => NumericType::F64,
        _ => NumericType::Integer(FixedInteger::from_decimal_spelling(name)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_float_row_requires_its_complete_signature() {
        for (row_index, (name, arity, _, _, _)) in FLOAT_OPERATIONS.iter().enumerate() {
            assert!(
                !FLOAT_OPERATIONS[..row_index]
                    .iter()
                    .any(|row| row.0 == *name)
            );
            assert_eq!(
                numerics::float_semantics_catalog::FloatSemanticOperation::named_call_arity(name),
                Some(*arity),
                "{name}",
            );
            for namespace in ["F32", "F64"] {
                let row = NamedNumericIntrinsic::resolve(namespace, name, ArithmeticDomain::Exact)
                    .expect("catalog operation resolves");
                let bytes = row
                    .execution
                    .canonical_encoding()
                    .expect("closed wire meaning");
                assert_eq!(
                    NumericIntrinsic::from_canonical_encoding(bytes),
                    Some(row.execution)
                );
                let parameters = vec![Some(row.operand); *arity];
                assert!(
                    row.accepts(parameters.iter().copied(), Some(row.result)),
                    "{namespace}::{name}"
                );
                assert!(!row.accepts(parameters[..arity - 1].iter().copied(), Some(row.result)));
                let mut excess = parameters.clone();
                excess.push(Some(row.operand));
                assert!(!row.accepts(excess.into_iter(), Some(row.result)));
                let mut wrong_operand = parameters.clone();
                wrong_operand[0] = Some(NumericType::Integer(FixedInteger::signed(32)));
                assert!(!row.accepts(wrong_operand.into_iter(), Some(row.result)));
                assert!(!row.accepts(
                    parameters.iter().copied(),
                    Some(NumericResult::Primitive(NumericType::Integer(
                        FixedInteger::signed(32)
                    )))
                ));
                assert!(!row.accepts(parameters.iter().copied(), None));
            }
        }
    }

    #[test]
    fn unusual_width_numeric_intrinsics_retain_exact_conversion_meaning() {
        for (namespace, name) in [("F32", "from_u13"), ("I257", "from_f64")] {
            let row = NamedNumericIntrinsic::resolve(namespace, name, ArithmeticDomain::Exact)
                .expect("numeric signatures retain arbitrary integer widths");
            assert_eq!(
                NumericIntrinsic::from_canonical_encoding(
                    row.execution.canonical_encoding().unwrap()
                ),
                Some(row.execution)
            );
            assert!(row.accepts([Some(row.operand)].into_iter(), Some(row.result)));
            assert!(!row.accepts([None].into_iter(), Some(row.result)));
        }
    }

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
        assert!(
            NumericType::Integer(FixedInteger::unsigned(13))
                .scalar_type()
                .is_some()
        );
        assert!(
            NumericType::Integer(FixedInteger::unsigned(257))
                .scalar_type()
                .is_none()
        );
    }

    #[test]
    fn conversions_bind_both_carriers_and_the_result_domain() {
        for (namespace, name, domain, source, target, expected_domain) in [
            (
                "F32",
                "from_i64",
                ArithmeticDomain::Wrapping,
                NumericType::Integer(FixedInteger::signed(64)),
                NumericType::F32,
                ArithmeticDomain::Exact,
            ),
            (
                "F64",
                "from_f32",
                ArithmeticDomain::Exact,
                NumericType::F32,
                NumericType::F64,
                ArithmeticDomain::Exact,
            ),
            (
                "I32",
                "from_f64",
                ArithmeticDomain::Saturating,
                NumericType::F64,
                NumericType::Integer(FixedInteger::signed(32)),
                ArithmeticDomain::Saturating,
            ),
            (
                "U8",
                "from_f32",
                ArithmeticDomain::Trapping,
                NumericType::F32,
                NumericType::Integer(FixedInteger::unsigned(8)),
                ArithmeticDomain::Trapping,
            ),
        ] {
            let row = NamedNumericIntrinsic::resolve(namespace, name, domain).expect("conversion");
            assert_eq!(
                NumericIntrinsic::from_canonical_encoding(
                    row.execution.canonical_encoding().unwrap()
                ),
                Some(row.execution)
            );
            assert_eq!(
                row.execution,
                NumericIntrinsic::Conversion {
                    source,
                    target,
                    domain: expected_domain,
                }
            );
            assert!(row.accepts([Some(row.operand)].into_iter(), Some(row.result)));
            assert!(!row.accepts([None].into_iter(), Some(row.result)));
            assert!(!row.accepts(
                [Some(row.operand)].into_iter(),
                Some(NumericResult::Boolean)
            ));
        }
    }

    #[test]
    fn unsupported_names_carriers_and_domains_have_no_execution() {
        for (namespace, name, domain) in [
            ("F32", "from_f32", ArithmeticDomain::Exact),
            ("F64", "from_bool", ArithmeticDomain::Exact),
            ("I32", "from_u32", ArithmeticDomain::Exact),
            ("I32", "from_f64", ArithmeticDomain::Wrapping),
            ("I32", "negate", ArithmeticDomain::Exact),
            ("Float", "minimum", ArithmeticDomain::Exact),
            ("F32", "add_toward_nearest", ArithmeticDomain::Exact),
            ("F32", "minimum_lookalike", ArithmeticDomain::Exact),
        ] {
            assert!(
                NamedNumericIntrinsic::resolve(namespace, name, domain).is_none(),
                "{namespace}::{name}"
            );
        }
    }

    #[test]
    fn result_categories_and_rounding_are_distinct_execution_meanings() {
        let resolve = |name| {
            NamedNumericIntrinsic::resolve("F32", name, ArithmeticDomain::Exact).expect("operation")
        };
        assert_eq!(
            resolve("negate").execution,
            NumericIntrinsic::Negation(FloatFormat::F32)
        );
        assert_eq!(
            resolve("negate").result,
            NumericResult::Primitive(NumericType::F32)
        );
        assert_eq!(resolve("is_nan").result, NumericResult::Boolean);
        assert_eq!(resolve("classify").result, NumericResult::FloatClass);
        for (name, function) in [
            (
                "multiply_then_add",
                BuiltinFunction::FloatMultiplyThenAddF32,
            ),
            (
                "fused_multiply_add",
                BuiltinFunction::FloatFusedMultiplyAddF32,
            ),
            (
                "fused_multiply_add_toward_zero",
                BuiltinFunction::FloatFusedMultiplyAddTowardZeroF32,
            ),
            (
                "square_root_toward_positive",
                BuiltinFunction::FloatSqrtTowardPositiveF32,
            ),
            (
                "divide_toward_negative",
                BuiltinFunction::FloatDivideTowardNegativeF32,
            ),
        ] {
            assert_eq!(
                resolve(name).execution,
                NumericIntrinsic::Builtin {
                    function,
                    format: FloatFormat::F32
                }
            );
        }
    }
}
