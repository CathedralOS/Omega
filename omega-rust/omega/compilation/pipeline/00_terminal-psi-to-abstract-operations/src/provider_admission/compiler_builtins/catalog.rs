//! The one conversion from an intrinsic identity to the local
//! `CompilerBuiltinExecution` catalog ([boundary calls](../../../../../../../../wiki/spec/terminal-psi/boundary_calls.md),
//! compiler-builtin settlement). `SettledCompilerBuiltin` has no other
//! constructor, so provider proposals and private access lifecycles both
//! settle through this exhaustive mapping instead of a policy guard beside an
//! independently built catalog entry.

use effects::CompilerIntrinsicExecutionIdentity;
use symbols::BuiltinFunction;
use target_operations::CompilerBuiltinExecution;

/// One catalog entry the mapping returned for an intrinsic identity.
///
/// ```compile_fail
/// let forged = terminal_psi_to_abstract_operations::SettledCompilerBuiltin(
///     target_operations::CompilerBuiltinExecution::HostedReadByte,
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettledCompilerBuiltin(CompilerBuiltinExecution);

impl SettledCompilerBuiltin {
    pub(crate) fn from_identity(
        identity: CompilerIntrinsicExecutionIdentity,
        boundary: semantic_vocabulary::BoundaryMachineId,
        boundary_format: Option<semantic_vocabulary::IeeeFloatFormat>,
    ) -> Option<Self> {
        compiler_builtin_execution(identity, boundary, boundary_format).map(Self)
    }

    pub const fn execution(self) -> CompilerBuiltinExecution {
        self.0
    }
}

/// One directed square root on one IEEE format. The mode is the operation,
/// same as directed binary arithmetic, so nearest square root cannot satisfy it.
fn nearest_directed_square_root(
    rounding: target_operations::IeeeFloatRoundingDirection,
    format: semantic_vocabulary::IeeeFloatFormat,
) -> CompilerBuiltinExecution {
    nearest_arithmetic(
        target_operations::IeeeFloatArithmeticOperation::DirectedSquareRoot { rounding },
        format,
    )
}

/// One directed binary rounding mode on one IEEE format realized as the
/// nearest-neighbor arithmetic builtin.
fn nearest_directed_binary(
    operation: target_operations::IeeeFloatBinaryOperation,
    rounding: target_operations::IeeeFloatRoundingDirection,
    format: semantic_vocabulary::IeeeFloatFormat,
) -> CompilerBuiltinExecution {
    nearest_arithmetic(
        target_operations::IeeeFloatArithmeticOperation::DirectedBinary {
            operation,
            rounding,
        },
        format,
    )
}

/// One non-decomposed float arithmetic builtin on one IEEE format.
fn nearest_arithmetic(
    operation: target_operations::IeeeFloatArithmeticOperation,
    format: semantic_vocabulary::IeeeFloatFormat,
) -> CompilerBuiltinExecution {
    CompilerBuiltinExecution::NearestIeeeFloatArithmetic { operation, format }
}

/// Format-bearing builtin symbols name an operation, a directed mode, and
/// one IEEE format. `nearest_arithmetic`, `nearest_directed_binary`, and
/// `nearest_directed_square_root` already build that execution row.
fn format_bearing_builtin(function: BuiltinFunction) -> Option<CompilerBuiltinExecution> {
    use semantic_vocabulary::IeeeFloatFormat::{Binary32, Binary64};
    use symbols::BuiltinFunction as Function;
    use target_operations::IeeeFloatArithmeticOperation as Arithmetic;
    use target_operations::IeeeFloatBinaryOperation as Binary;
    use target_operations::IeeeFloatRoundingDirection as Rounding;

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

/// The native realization of one intrinsic identity at `boundary`, if the
/// local catalog has one. `boundary_format` is the Psi-published IEEE
/// format, which the format-free custody identities need; admission rejoins
/// the complete boundary signature independently.
fn compiler_builtin_execution(
    identity: CompilerIntrinsicExecutionIdentity,
    boundary: semantic_vocabulary::BoundaryMachineId,
    boundary_format: Option<semantic_vocabulary::IeeeFloatFormat>,
) -> Option<CompilerBuiltinExecution> {
    use target_operations::IeeeFloatArithmeticOperation as Arithmetic;
    let ieee_format = |format| match format {
        numerics::literals::FloatFormat::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
        numerics::literals::FloatFormat::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
    };
    let (operation, format) = match identity {
        CompilerIntrinsicExecutionIdentity::AccessLifecycle(operation) => {
            return Some(CompilerBuiltinExecution::AccessLifecycle {
                boundary,
                operation,
            });
        }
        CompilerIntrinsicExecutionIdentity::ProgramImageRetainedSuffixStart => {
            return Some(CompilerBuiltinExecution::ProgramImageRetainedSuffixStart);
        }
        CompilerIntrinsicExecutionIdentity::ProgramStorageEntry => {
            return Some(CompilerBuiltinExecution::ProgramStorageEntry);
        }
        CompilerIntrinsicExecutionIdentity::HostedExitProcessI32 => {
            return Some(CompilerBuiltinExecution::HostedExitProcessI32);
        }
        CompilerIntrinsicExecutionIdentity::HostedWriteByteI32 => {
            return Some(CompilerBuiltinExecution::HostedWriteByteI32);
        }
        CompilerIntrinsicExecutionIdentity::AccessorScalarAccess {
            contract,
            placement_commitment,
            access_slot,
            operation,
        } => {
            return Some(CompilerBuiltinExecution::AccessorScalarAccess {
                contract,
                placement_commitment,
                access_slot,
                operation: match operation {
                    effects::CompilerAccessorScalarOperation::Read => {
                        terminal_psi::TerminalAccessorKind::Read
                    }
                    effects::CompilerAccessorScalarOperation::Take => {
                        terminal_psi::TerminalAccessorKind::Take
                    }
                    effects::CompilerAccessorScalarOperation::Write => {
                        terminal_psi::TerminalAccessorKind::Write
                    }
                },
            });
        }
        CompilerIntrinsicExecutionIdentity::HostedReadByte => {
            return Some(CompilerBuiltinExecution::HostedReadByte);
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format } => {
            // Comparisons demanded as boundary calls realize as the same
            // IEEE relation the emitted Terminal comparison selects; a
            // comparison selected on a use site still settles its emitted
            // `IeeeFloatCompare`, never this row.
            use effects::CompilerPrimitiveFloatBinaryOperation as Primitive;
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
            // Format-bearing identities must not borrow an unrelated boundary's
            // result format. Complete signature admission below remains required.
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
            use effects::CompilerNumericType as Numeric;
            use numerics::arithmetic::ArithmeticDomain;
            use target_operations::{IeeeFloatIntegerCarrier as Carrier, IeeeFloatToIntegerDomain};
            let float = |numeric| match numeric {
                Numeric::F32 => Some(semantic_vocabulary::IeeeFloatFormat::Binary32),
                Numeric::F64 => Some(semantic_vocabulary::IeeeFloatFormat::Binary64),
                _ => None,
            };
            let carrier = |numeric| match numeric {
                Numeric::I8 => Some(Carrier::I8),
                Numeric::I16 => Some(Carrier::I16),
                Numeric::I32 => Some(Carrier::I32),
                Numeric::I64 => Some(Carrier::I64),
                Numeric::U8 => Some(Carrier::U8),
                Numeric::U16 => Some(Carrier::U16),
                Numeric::U32 => Some(Carrier::U32),
                Numeric::U64 => Some(Carrier::U64),
                _ => None,
            };
            // Conversions into a float format are total nearest-even Exact
            // requirements. Conversions to an integer truncate: Exact and
            // Saturating share clamped truncation on the Exact proof's valid
            // inputs. Trapping retains its invalid-input crash mechanism.
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
        CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison { .. } => return None,
    };
    Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic { operation, format })
}
