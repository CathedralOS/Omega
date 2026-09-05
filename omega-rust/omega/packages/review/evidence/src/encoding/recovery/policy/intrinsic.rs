//! Exact closed execution vocabulary, without evaluator or native receipts.

#[cfg(test)]
mod tests;

use super::{Error, reader::Reader};
use crate::record::PackageReviewCompilerIntrinsicExecution;
use numerics::{arithmetic::ArithmeticDomain, literals::FloatFormat};
use provider_planning::plans::{CompilerNumericType, CompilerPrimitiveFloatBinaryOperation};

pub(super) fn execution(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewCompilerIntrinsicExecution, Error> {
    use PackageReviewCompilerIntrinsicExecution as Execution;
    Ok(match reader.byte()? {
        0 => Execution::BuiltinFunction(
            symbols::BuiltinFunction::from_ordinal(usize::from(reader.u16()?))
                .ok_or(Error::InvalidTag)?,
        ),
        1 => Execution::NamedFloatNegation(float_format(reader)?),
        2 => Execution::NamedFloatConversion {
            source: numeric_type(reader)?,
            target: numeric_type(reader)?,
            domain: arithmetic_domain(reader)?,
        },
        3 => Execution::PrimitiveFloatBinary {
            operation: float_binary(reader)?,
            format: float_format(reader)?,
        },
        4 => Execution::LinuxExitGroupI32,
        5 => Execution::LinuxWriteByteI32,
        6 => Execution::LinuxReadByte,
        _ => return Err(Error::InvalidTag),
    })
}

fn float_format(reader: &mut Reader<'_>) -> Result<FloatFormat, Error> {
    Ok(match reader.byte()? {
        0 => FloatFormat::F32,
        1 => FloatFormat::F64,
        _ => return Err(Error::InvalidTag),
    })
}

fn float_binary(reader: &mut Reader<'_>) -> Result<CompilerPrimitiveFloatBinaryOperation, Error> {
    use CompilerPrimitiveFloatBinaryOperation as Operation;
    Ok(match reader.byte()? {
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
        _ => return Err(Error::InvalidTag),
    })
}

fn numeric_type(reader: &mut Reader<'_>) -> Result<CompilerNumericType, Error> {
    use CompilerNumericType as Numeric;
    Ok(match reader.byte()? {
        0 => Numeric::I8,
        1 => Numeric::I16,
        2 => Numeric::I32,
        3 => Numeric::I64,
        4 => Numeric::U8,
        5 => Numeric::U16,
        6 => Numeric::U32,
        7 => Numeric::U64,
        8 => Numeric::F32,
        9 => Numeric::F64,
        _ => return Err(Error::InvalidTag),
    })
}

fn arithmetic_domain(reader: &mut Reader<'_>) -> Result<ArithmeticDomain, Error> {
    Ok(match reader.byte()? {
        0 => ArithmeticDomain::Exact,
        1 => ArithmeticDomain::Wrapping,
        2 => ArithmeticDomain::Saturating,
        3 => ArithmeticDomain::Trapping,
        _ => return Err(Error::InvalidTag),
    })
}
