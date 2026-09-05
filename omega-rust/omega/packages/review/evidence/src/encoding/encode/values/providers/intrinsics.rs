use crate::encoding::{PackageReviewEncodingError, encode::encoder::Encoder};
use crate::record::PackageReviewCompilerIntrinsicExecution;

pub(crate) fn encode_compiler_intrinsic_execution(
    encoder: &mut Encoder,
    execution: &PackageReviewCompilerIntrinsicExecution,
) -> Result<(), PackageReviewEncodingError> {
    match execution {
        PackageReviewCompilerIntrinsicExecution::LinuxExitGroupI32 => {
            encoder.tag("linux_exit_group_i32", 4);
        }
        PackageReviewCompilerIntrinsicExecution::LinuxWriteByteI32 => {
            encoder.tag("linux_write_byte_i32", 5);
        }
        PackageReviewCompilerIntrinsicExecution::LinuxReadByte => {
            encoder.tag("linux_read_byte", 6);
        }
        PackageReviewCompilerIntrinsicExecution::BuiltinFunction(function) => {
            encoder.tag("builtin_function", 0);
            encoder.field("ordinal", |encoder| {
                encoder.u16(u16::try_from(function.ordinal()).map_err(|_| {
                    PackageReviewEncodingError::new(
                        "compiler builtin-function ordinal exceeds the portable encoding range",
                    )
                })?);
                Ok(())
            })?;
        }
        PackageReviewCompilerIntrinsicExecution::PrimitiveFloatBinary { operation, format } => {
            encoder.tag("primitive_float_binary", 3);
            encoder.field("operation", |encoder| {
                encode_primitive_float_binary_operation(encoder, *operation);
                Ok(())
            })?;
            encoder.field("format", |encoder| {
                encode_float_format(encoder, *format);
                Ok(())
            })?;
        }
        PackageReviewCompilerIntrinsicExecution::NamedFloatNegation(format) => {
            encoder.tag("named_float_negation", 1);
            encoder.field("format", |encoder| {
                encode_float_format(encoder, *format);
                Ok(())
            })?;
        }
        PackageReviewCompilerIntrinsicExecution::NamedFloatConversion {
            source,
            target,
            domain,
        } => {
            encoder.tag("named_float_conversion", 2);
            encoder.field("source", |encoder| {
                encode_compiler_numeric_type(encoder, *source);
                Ok(())
            })?;
            encoder.field("target", |encoder| {
                encode_compiler_numeric_type(encoder, *target);
                Ok(())
            })?;
            encoder.field("domain", |encoder| {
                encode_compiler_arithmetic_domain(encoder, *domain);
                Ok(())
            })?;
        }
    }
    Ok(())
}

fn encode_primitive_float_binary_operation(
    encoder: &mut Encoder,
    operation: omega_provider_planning::plans::CompilerPrimitiveFloatBinaryOperation,
) {
    use omega_provider_planning::plans::CompilerPrimitiveFloatBinaryOperation;

    match operation {
        CompilerPrimitiveFloatBinaryOperation::Add => encoder.tag("add", 0),
        CompilerPrimitiveFloatBinaryOperation::Subtract => encoder.tag("subtract", 1),
        CompilerPrimitiveFloatBinaryOperation::Multiply => encoder.tag("multiply", 2),
        CompilerPrimitiveFloatBinaryOperation::Divide => encoder.tag("divide", 3),
        CompilerPrimitiveFloatBinaryOperation::Equal => encoder.tag("equal", 4),
        CompilerPrimitiveFloatBinaryOperation::NotEqual => encoder.tag("not_equal", 5),
        CompilerPrimitiveFloatBinaryOperation::Less => encoder.tag("less", 6),
        CompilerPrimitiveFloatBinaryOperation::LessOrEqual => encoder.tag("less_or_equal", 7),
        CompilerPrimitiveFloatBinaryOperation::Greater => encoder.tag("greater", 8),
        CompilerPrimitiveFloatBinaryOperation::GreaterOrEqual => encoder.tag("greater_or_equal", 9),
    };
}

fn encode_float_format(encoder: &mut Encoder, format: psi_numerics::literals::FloatFormat) {
    match format {
        psi_numerics::literals::FloatFormat::F32 => encoder.tag("f32", 0),
        psi_numerics::literals::FloatFormat::F64 => encoder.tag("f64", 1),
    };
}

fn encode_compiler_numeric_type(
    encoder: &mut Encoder,
    numeric_type: omega_provider_planning::plans::CompilerNumericType,
) {
    use omega_provider_planning::plans::CompilerNumericType;

    match numeric_type {
        CompilerNumericType::I8 => encoder.tag("i8", 0),
        CompilerNumericType::I16 => encoder.tag("i16", 1),
        CompilerNumericType::I32 => encoder.tag("i32", 2),
        CompilerNumericType::I64 => encoder.tag("i64", 3),
        CompilerNumericType::U8 => encoder.tag("u8", 4),
        CompilerNumericType::U16 => encoder.tag("u16", 5),
        CompilerNumericType::U32 => encoder.tag("u32", 6),
        CompilerNumericType::U64 => encoder.tag("u64", 7),
        CompilerNumericType::F32 => encoder.tag("f32", 8),
        CompilerNumericType::F64 => encoder.tag("f64", 9),
    };
}

fn encode_compiler_arithmetic_domain(
    encoder: &mut Encoder,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
) {
    use psi_numerics::arithmetic::ArithmeticDomain;

    match domain {
        ArithmeticDomain::Exact => encoder.tag("exact", 0),
        ArithmeticDomain::Wrapping => encoder.tag("wrapping", 1),
        ArithmeticDomain::Saturating => encoder.tag("saturating", 2),
        ArithmeticDomain::Trapping => encoder.tag("trapping", 3),
    };
}
