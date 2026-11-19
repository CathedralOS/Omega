//! Map a selected numeric identity into an executable IEEE operation and format.
use language_core::BuiltinFunction;
use native_intrinsics::CompilerIntrinsicExecutionIdentity;
use target_vocabulary::NearestIeeeFloatArithmeticRealization;
fn nearest_directed_square_root(
    rounding: target_vocabulary::IeeeFloatRoundingDirection,
    format: semantic_vocabulary::IeeeFloatFormat,
) -> NearestIeeeFloatArithmeticRealization {
    nearest_arithmetic(
        target_vocabulary::IeeeFloatArithmeticOperation::DirectedSquareRoot { rounding },
        format,
    )
}

fn nearest_directed_binary(
    operation: target_vocabulary::IeeeFloatBinaryOperation,
    rounding: target_vocabulary::IeeeFloatRoundingDirection,
    format: semantic_vocabulary::IeeeFloatFormat,
) -> NearestIeeeFloatArithmeticRealization {
    nearest_arithmetic(
        target_vocabulary::IeeeFloatArithmeticOperation::DirectedBinary {
            operation,
            rounding,
        },
        format,
    )
}

fn nearest_arithmetic(
    operation: target_vocabulary::IeeeFloatArithmeticOperation,
    format: semantic_vocabulary::IeeeFloatFormat,
) -> NearestIeeeFloatArithmeticRealization {
    NearestIeeeFloatArithmeticRealization {
        operation,
        format,
        x86_scalar_fma: None,
    }
}

fn format_bearing_builtin(
    function: BuiltinFunction,
) -> Option<NearestIeeeFloatArithmeticRealization> {
    use language_core::BuiltinFunction as Function;
    use semantic_vocabulary::IeeeFloatFormat::{Binary32, Binary64};
    use target_vocabulary::IeeeFloatArithmeticOperation as Arithmetic;
    use target_vocabulary::IeeeFloatBinaryOperation as Binary;
    use target_vocabulary::IeeeFloatRoundingDirection as Rounding;

    let classify = Arithmetic::Classify;
    let fused = Arithmetic::FusedMultiplyAdd;
    let multiply_add = Arithmetic::MultiplyThenAdd;
    let directed =
        |operation, rounding, format| nearest_directed_binary(operation, rounding, format);
    let square_root = |rounding, format| nearest_directed_square_root(rounding, format);
    let arithmetic = |operation, format| nearest_arithmetic(operation, format);
    Some(match function {
        Function::FloatClassifyF32 => arithmetic(classify, Binary32),
        Function::FloatClassifyF64 => arithmetic(classify, Binary64),
        Function::FloatAddTowardZeroF32 => directed(Binary::Add, Rounding::TowardZero, Binary32),
        Function::FloatAddTowardZeroF64 => directed(Binary::Add, Rounding::TowardZero, Binary64),
        Function::FloatAddTowardPositiveF32 => {
            directed(Binary::Add, Rounding::TowardPositive, Binary32)
        }
        Function::FloatAddTowardPositiveF64 => {
            directed(Binary::Add, Rounding::TowardPositive, Binary64)
        }
        Function::FloatAddTowardNegativeF32 => {
            directed(Binary::Add, Rounding::TowardNegative, Binary32)
        }
        Function::FloatAddTowardNegativeF64 => {
            directed(Binary::Add, Rounding::TowardNegative, Binary64)
        }
        Function::FloatSubtractTowardZeroF32 => {
            directed(Binary::Subtract, Rounding::TowardZero, Binary32)
        }
        Function::FloatSubtractTowardZeroF64 => {
            directed(Binary::Subtract, Rounding::TowardZero, Binary64)
        }
        Function::FloatSubtractTowardPositiveF32 => {
            directed(Binary::Subtract, Rounding::TowardPositive, Binary32)
        }
        Function::FloatSubtractTowardPositiveF64 => {
            directed(Binary::Subtract, Rounding::TowardPositive, Binary64)
        }
        Function::FloatSubtractTowardNegativeF32 => {
            directed(Binary::Subtract, Rounding::TowardNegative, Binary32)
        }
        Function::FloatSubtractTowardNegativeF64 => {
            directed(Binary::Subtract, Rounding::TowardNegative, Binary64)
        }
        Function::FloatMultiplyTowardZeroF32 => {
            directed(Binary::Multiply, Rounding::TowardZero, Binary32)
        }
        Function::FloatMultiplyTowardZeroF64 => {
            directed(Binary::Multiply, Rounding::TowardZero, Binary64)
        }
        Function::FloatMultiplyTowardPositiveF32 => {
            directed(Binary::Multiply, Rounding::TowardPositive, Binary32)
        }
        Function::FloatMultiplyTowardPositiveF64 => {
            directed(Binary::Multiply, Rounding::TowardPositive, Binary64)
        }
        Function::FloatMultiplyTowardNegativeF32 => {
            directed(Binary::Multiply, Rounding::TowardNegative, Binary32)
        }
        Function::FloatMultiplyTowardNegativeF64 => {
            directed(Binary::Multiply, Rounding::TowardNegative, Binary64)
        }
        Function::FloatDivideTowardZeroF32 => {
            directed(Binary::Divide, Rounding::TowardZero, Binary32)
        }
        Function::FloatDivideTowardZeroF64 => {
            directed(Binary::Divide, Rounding::TowardZero, Binary64)
        }
        Function::FloatDivideTowardPositiveF32 => {
            directed(Binary::Divide, Rounding::TowardPositive, Binary32)
        }
        Function::FloatDivideTowardPositiveF64 => {
            directed(Binary::Divide, Rounding::TowardPositive, Binary64)
        }
        Function::FloatDivideTowardNegativeF32 => {
            directed(Binary::Divide, Rounding::TowardNegative, Binary32)
        }
        Function::FloatDivideTowardNegativeF64 => {
            directed(Binary::Divide, Rounding::TowardNegative, Binary64)
        }
        Function::FloatSqrtTowardZeroF32 => square_root(Rounding::TowardZero, Binary32),
        Function::FloatSqrtTowardZeroF64 => square_root(Rounding::TowardZero, Binary64),
        Function::FloatSqrtTowardPositiveF32 => square_root(Rounding::TowardPositive, Binary32),
        Function::FloatSqrtTowardPositiveF64 => square_root(Rounding::TowardPositive, Binary64),
        Function::FloatSqrtTowardNegativeF32 => square_root(Rounding::TowardNegative, Binary32),
        Function::FloatSqrtTowardNegativeF64 => square_root(Rounding::TowardNegative, Binary64),
        Function::FloatFusedMultiplyAddF32 => arithmetic(fused, Binary32),
        Function::FloatFusedMultiplyAddF64 => arithmetic(fused, Binary64),
        Function::FloatMultiplyThenAddF32 => arithmetic(multiply_add, Binary32),
        Function::FloatMultiplyThenAddF64 => arithmetic(multiply_add, Binary64),
        _ => return None,
    })
}

pub fn catalog_realization(
    identity: CompilerIntrinsicExecutionIdentity,
    boundary_format: Option<semantic_vocabulary::IeeeFloatFormat>,
) -> Option<NearestIeeeFloatArithmeticRealization> {
    use target_vocabulary::IeeeFloatArithmeticOperation as Arithmetic;
    let ieee_format = |format| match format {
        numeric_literals::FloatFormat::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
        numeric_literals::FloatFormat::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
    };
    let (operation, format) = match identity {
        CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format } => {
            use native_intrinsics::CompilerPrimitiveFloatBinaryOperation as Primitive;
            use semantic_vocabulary::IeeeFloatComparisonOperation as Relation;
            let operation = match operation {
                Primitive::Add => Arithmetic::Add,
                Primitive::Subtract => Arithmetic::Subtract,
                Primitive::Multiply => Arithmetic::Multiply,
                Primitive::Divide => Arithmetic::Divide,
                Primitive::Equal => Arithmetic::Compare(Relation::Equal),
                Primitive::NotEqual => Arithmetic::Compare(Relation::NotEqual),
                Primitive::Less => Arithmetic::Compare(Relation::Less),
                Primitive::LessOrEqual => Arithmetic::Compare(Relation::LessOrEqual),
                Primitive::Greater => Arithmetic::Compare(Relation::Greater),
                Primitive::GreaterOrEqual => Arithmetic::Compare(Relation::GreaterOrEqual),
            };
            (operation, ieee_format(format))
        }
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function) => {
            if let Some(execution) = format_bearing_builtin(function) {
                return Some(execution);
            }

            let operation = match function {
                BuiltinFunction::FloatMinimum => Arithmetic::Minimum,
                BuiltinFunction::FloatMaximum => Arithmetic::Maximum,
                BuiltinFunction::FloatSquareRoot => Arithmetic::SquareRoot,
                BuiltinFunction::FloatIsNan => Arithmetic::IsNan,
                BuiltinFunction::FloatIsInfinite => Arithmetic::IsInfinite,
                BuiltinFunction::FloatIsFinite => Arithmetic::IsFinite,
                BuiltinFunction::FloatIsNormal => Arithmetic::IsNormal,
                BuiltinFunction::FloatIsSubnormal => Arithmetic::IsSubnormal,
                _ => return None,
            };
            (operation, boundary_format?)
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatNegation(format) => {
            (Arithmetic::Negate, ieee_format(format))
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
            source,
            target,
            domain,
        } => {
            use native_intrinsics::CompilerNumericType as Numeric;
            use numerics::arithmetic::ArithmeticDomain;
            use target_vocabulary::{IeeeFloatIntegerCarrier as Carrier, IeeeFloatToIntegerDomain};
            let float = |numeric| match numeric {
                Numeric::F32 => Some(semantic_vocabulary::IeeeFloatFormat::Binary32),
                Numeric::F64 => Some(semantic_vocabulary::IeeeFloatFormat::Binary64),
                _ => None,
            };
            let carrier = |numeric| {
                let Numeric::Integer(integer) = numeric else {
                    return None;
                };
                // Execution admits only an exact carrier in the target operation catalog.
                Carrier::ALL.into_iter().find(|candidate| {
                    candidate.is_signed() == integer.is_signed()
                        && u64::from(candidate.bits()) == integer.width()
                })
            };
            match (float(source), float(target), domain) {
                (Some(from), Some(format), ArithmeticDomain::Exact) if from != format => {
                    (Arithmetic::ConvertFormat, format)
                }
                (None, Some(format), ArithmeticDomain::Exact) => {
                    (Arithmetic::FromInteger(carrier(source)?), format)
                }
                (
                    Some(format),
                    None,
                    ArithmeticDomain::Exact
                    | ArithmeticDomain::Saturating
                    | ArithmeticDomain::Trapping,
                ) => (
                    Arithmetic::TruncateToInteger {
                        target: carrier(target)?,
                        domain: match domain {
                            ArithmeticDomain::Exact => IeeeFloatToIntegerDomain::Exact,
                            ArithmeticDomain::Saturating => IeeeFloatToIntegerDomain::Saturating,
                            ArithmeticDomain::Trapping => IeeeFloatToIntegerDomain::Trapping,
                            _ => return None,
                        },
                    },
                    format,
                ),
                _ => return None,
            }
        }
        _ => return None,
    };
    Some(NearestIeeeFloatArithmeticRealization {
        operation,
        format,
        x86_scalar_fma: None,
    })
}
