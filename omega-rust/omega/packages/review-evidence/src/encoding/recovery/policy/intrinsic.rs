// Exact closed execution vocabulary recovery without evaluator or native receipts.

use super::{Error, reader::Reader};
use crate::encoding::intrinsic_tags::IntrinsicTag;
use crate::record::PackageReviewCompilerIntrinsicExecution;
use numerics::{arithmetic::ArithmeticDomain, literals::FloatFormat};
use provider_planning::{
    CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation,
};

pub(in crate::encoding::recovery) fn execution(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewCompilerIntrinsicExecution, Error> {
    use PackageReviewCompilerIntrinsicExecution as Execution;
    let tag = IntrinsicTag::from_code(reader.byte()?).ok_or(Error::InvalidTag)?;
    Ok(match tag {
        IntrinsicTag::BuiltinFunction => Execution::BuiltinFunction(
            symbols::BuiltinFunction::from_ordinal(usize::from(reader.u16()?))
                .ok_or(Error::InvalidTag)?,
        ),
        IntrinsicTag::NamedFloatNegation => Execution::NamedFloatNegation(float_format(reader)?),
        IntrinsicTag::NamedFloatConversion => Execution::NamedFloatConversion {
            source: numeric_type(reader)?,
            target: numeric_type(reader)?,
            domain: arithmetic_domain(reader)?,
        },
        IntrinsicTag::PrimitiveFloatBinary => Execution::PrimitiveFloatBinary {
            operation: float_binary(reader)?,
            format: float_format(reader)?,
        },
        IntrinsicTag::HostedExitProcessI32 => Execution::HostedExitProcessI32,
        IntrinsicTag::HostedWriteByteI32 => Execution::HostedWriteByteI32,
        IntrinsicTag::HostedReadByte => Execution::HostedReadByte,
        IntrinsicTag::ProgramImageRetainedSuffixStart => Execution::ProgramImageRetainedSuffixStart,
        IntrinsicTag::ProgramStorageEntry => Execution::ProgramStorageEntry,
        IntrinsicTag::TaskRegisterActivate => Execution::TaskRegisterActivate,
        IntrinsicTag::TaskRegisterRead => Execution::TaskRegisterRead,
        IntrinsicTag::SegmentReload => Execution::SegmentReload(
            terminal_psi::SegmentReload::from_code(reader.byte()?).ok_or(Error::InvalidTag)?,
        ),
        IntrinsicTag::SegmentRead => Execution::SegmentRead(
            terminal_psi::SegmentRegister::from_code(reader.byte()?).ok_or(Error::InvalidTag)?,
        ),
        IntrinsicTag::EmptyRunRelease => Execution::EmptyRunRelease,
        IntrinsicTag::StaticInterrupt => {
            use language_core::static_interrupts::StaticInterruptOperation;
            let operation_code = reader.byte()?;
            Execution::StaticInterrupt(
                StaticInterruptOperation::ALL
                    .into_iter()
                    .find(|operation| operation.code() == operation_code)
                    .ok_or(Error::InvalidTag)?,
            )
        }
        IntrinsicTag::AccessLifecycle => {
            use terminal_psi::TerminalAccessLifecycleOperation as Operation;
            Execution::AccessLifecycle(super::tag!(reader => {
                0 => Operation::Initialize,
                1 => Operation::MoveOut,
                2 => Operation::View,
                3 => Operation::Relocate,
            }))
        }
        IntrinsicTag::AccessorScalarAccess => {
            use effects::CompilerAccessorScalarOperation as Operation;
            Execution::AccessorScalarAccess {
                contract: reader.digest()?,
                placement_commitment: reader.digest()?,
                access_slot: reader.u32()?,
                operation: super::tag!(reader => {
                    0 => Operation::Read,
                    1 => Operation::Take,
                    2 => Operation::Write,
                }),
            }
        }
        IntrinsicTag::PrimitiveIntegerComparison => {
            let operation = integer_comparison(reader)?;
            let integer_type = numeric_type(reader)?;
            if integer_type.is_float() {
                return Err(Error::InvalidTag);
            }
            Execution::PrimitiveIntegerComparison {
                operation,
                integer_type,
            }
        }
    })
}

#[cfg(test)]
mod tests;

pub(in crate::encoding::recovery) fn integer_comparison(
    reader: &mut Reader<'_>,
) -> Result<CompilerPrimitiveIntegerComparisonOperation, Error> {
    use CompilerPrimitiveIntegerComparisonOperation as Operation;
    Ok(super::tag!(reader => {
        0 => Operation::Equal,
        1 => Operation::NotEqual,
        2 => Operation::Less,
        3 => Operation::LessOrEqual,
        4 => Operation::Greater,
        5 => Operation::GreaterOrEqual,
    }))
}

pub(in crate::encoding::recovery) fn float_format(
    reader: &mut Reader<'_>,
) -> Result<FloatFormat, Error> {
    Ok(super::tag!(reader => {
        0 => FloatFormat::F32,
        1 => FloatFormat::F64,
    }))
}

pub(in crate::encoding::recovery) fn float_binary(
    reader: &mut Reader<'_>,
) -> Result<CompilerPrimitiveFloatBinaryOperation, Error> {
    use CompilerPrimitiveFloatBinaryOperation as Operation;
    Ok(super::tag!(reader => {
        0 => Operation::Add,
        1 => Operation::Subtract,
        2 => Operation::Multiply,
        3 => Operation::Divide,
        4 => Operation::Equal,
        5 => Operation::NotEqual,
        6 => Operation::Less,
        7 => Operation::LessOrEqual,
        8 => Operation::Greater,
        9 => Operation::GreaterOrEqual,
    }))
}

pub(in crate::encoding::recovery) fn numeric_type(
    reader: &mut Reader<'_>,
) -> Result<CompilerNumericType, Error> {
    CompilerNumericType::from_canonical_encoding(reader.array()?).ok_or(Error::InvalidValue)
}

pub(in crate::encoding::recovery) fn arithmetic_domain(
    reader: &mut Reader<'_>,
) -> Result<ArithmeticDomain, Error> {
    Ok(super::tag!(reader => {
        0 => ArithmeticDomain::Exact,
        1 => ArithmeticDomain::Wrapping,
        2 => ArithmeticDomain::Saturating,
        3 => ArithmeticDomain::Trapping,
    }))
}
