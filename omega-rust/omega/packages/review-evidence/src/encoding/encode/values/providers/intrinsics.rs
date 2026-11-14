// Canonical encoding for compiler intrinsic executions and primitive numeric/float operations.

use crate::encoding::intrinsic_tags::IntrinsicTag;
use crate::encoding::{PackageReviewEncodingError, encode::encoder::Encoder};
use crate::record::PackageReviewCompilerIntrinsicExecution;
use numerics::arithmetic::ArithmeticDomain;
use terminal_psi::TerminalAccessLifecycleOperation;

pub(crate) fn encode_compiler_intrinsic_execution(
    encoder: &mut Encoder,
    execution: &PackageReviewCompilerIntrinsicExecution,
) -> Result<(), PackageReviewEncodingError> {
    let tag = IntrinsicTag::of(execution);
    encoder.tag(tag.label(), tag as u8);
    match execution {
        PackageReviewCompilerIntrinsicExecution::EmptyRunRelease
        | PackageReviewCompilerIntrinsicExecution::TaskRegisterActivate
        | PackageReviewCompilerIntrinsicExecution::TaskRegisterRead
        | PackageReviewCompilerIntrinsicExecution::ProgramStorageEntry
        | PackageReviewCompilerIntrinsicExecution::ProgramImageRetainedSuffixStart
        | PackageReviewCompilerIntrinsicExecution::HostedExitProcessI32
        | PackageReviewCompilerIntrinsicExecution::HostedWriteByteI32
        | PackageReviewCompilerIntrinsicExecution::HostedReadByte => {}
        PackageReviewCompilerIntrinsicExecution::StaticInterrupt(operation) => {
            encoder.field("operation", |encoder| {
                use language_core::static_interrupts::StaticInterruptOperation;
                encoder.tag(
                    match operation {
                        StaticInterruptOperation::Install => "install",
                        StaticInterruptOperation::Context => "context",
                        StaticInterruptOperation::BeginCompletion => "begin_completion",
                        StaticInterruptOperation::FinishCompletion => "finish_completion",
                    },
                    operation.code(),
                );
                Ok(())
            })?;
        }
        PackageReviewCompilerIntrinsicExecution::SegmentReload(kind) => {
            encoder.field("kind", |encoder| {
                encoder.tag(
                    match kind {
                        terminal_psi::SegmentReload::Code => "code",
                        terminal_psi::SegmentReload::Data => "data",
                    },
                    kind.code(),
                );
                Ok(())
            })?;
        }
        PackageReviewCompilerIntrinsicExecution::SegmentRead(register) => {
            encoder.field("register", |encoder| {
                encoder.tag(
                    match register {
                        terminal_psi::SegmentRegister::Code => "cs",
                        terminal_psi::SegmentRegister::Data => "ds",
                        terminal_psi::SegmentRegister::Extra => "es",
                        terminal_psi::SegmentRegister::Stack => "ss",
                        terminal_psi::SegmentRegister::Fs => "fs",
                        terminal_psi::SegmentRegister::Gs => "gs",
                    },
                    register.code(),
                );
                Ok(())
            })?;
        }
        PackageReviewCompilerIntrinsicExecution::AccessLifecycle(operation) => {
            encoder.field("operation", |encoder| {
                match operation {
                    TerminalAccessLifecycleOperation::Initialize => encoder.tag("initialize", 0),
                    TerminalAccessLifecycleOperation::MoveOut => encoder.tag("move_out", 1),
                    TerminalAccessLifecycleOperation::View => encoder.tag("view", 2),
                    TerminalAccessLifecycleOperation::Relocate => encoder.tag("relocate", 3),
                }
                Ok(())
            })?;
        }
        PackageReviewCompilerIntrinsicExecution::AccessorScalarAccess {
            contract,
            placement_commitment,
            access_slot,
            operation,
        } => {
            encoder.fixed_bytes_field("contract", contract)?;
            encoder.fixed_bytes_field("placement_commitment", placement_commitment)?;
            encoder.u32_field("access_slot", *access_slot)?;
            encoder.field("operation", |encoder| {
                match operation {
                    effects::CompilerAccessorScalarOperation::Read => encoder.tag("read", 0),
                    effects::CompilerAccessorScalarOperation::Take => encoder.tag("take", 1),
                    effects::CompilerAccessorScalarOperation::Write => encoder.tag("write", 2),
                }
                Ok(())
            })?;
        }
        PackageReviewCompilerIntrinsicExecution::BuiltinFunction(function) => {
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
            encoder.field_by(
                "operation",
                *operation,
                encode_primitive_float_binary_operation,
            )?;
            encoder.field_by("format", *format, encode_float_format)?;
        }
        PackageReviewCompilerIntrinsicExecution::PrimitiveIntegerComparison {
            operation,
            integer_type,
        } => {
            encoder.field_by(
                "operation",
                *operation,
                encode_primitive_integer_comparison_operation,
            )?;
            encoder.field_by("integer_type", *integer_type, encode_compiler_numeric_type)?;
        }
        PackageReviewCompilerIntrinsicExecution::NamedFloatNegation(format) => {
            encoder.field_by("format", *format, encode_float_format)?;
        }
        PackageReviewCompilerIntrinsicExecution::NamedFloatConversion {
            source,
            target,
            domain,
        } => {
            encoder.field_by("source", *source, encode_compiler_numeric_type)?;
            encoder.field_by("target", *target, encode_compiler_numeric_type)?;
            encoder.field_by("domain", *domain, encode_compiler_arithmetic_domain)?;
        }
    }
    Ok(())
}

fn encode_primitive_float_binary_operation(
    encoder: &mut Encoder,
    operation: provider_planning::CompilerPrimitiveFloatBinaryOperation,
) {
    use provider_planning::CompilerPrimitiveFloatBinaryOperation;

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

fn encode_primitive_integer_comparison_operation(
    encoder: &mut Encoder,
    operation: provider_planning::CompilerPrimitiveIntegerComparisonOperation,
) {
    use provider_planning::CompilerPrimitiveIntegerComparisonOperation;

    match operation {
        CompilerPrimitiveIntegerComparisonOperation::Equal => encoder.tag("equal", 0),
        CompilerPrimitiveIntegerComparisonOperation::NotEqual => encoder.tag("not_equal", 1),
        CompilerPrimitiveIntegerComparisonOperation::Less => encoder.tag("less", 2),
        CompilerPrimitiveIntegerComparisonOperation::LessOrEqual => encoder.tag("less_or_equal", 3),
        CompilerPrimitiveIntegerComparisonOperation::Greater => encoder.tag("greater", 4),
        CompilerPrimitiveIntegerComparisonOperation::GreaterOrEqual => {
            encoder.tag("greater_or_equal", 5)
        }
    };
}

fn encode_float_format(encoder: &mut Encoder, format: numerics::literals::FloatFormat) {
    match format {
        numerics::literals::FloatFormat::F32 => encoder.tag("f32", 0),
        numerics::literals::FloatFormat::F64 => encoder.tag("f64", 1),
    };
}

fn encode_compiler_numeric_type(
    encoder: &mut Encoder,
    numeric_type: provider_planning::CompilerNumericType,
) {
    encoder.fixed_bytes(&numeric_type.canonical_encoding());
}

fn encode_compiler_arithmetic_domain(encoder: &mut Encoder, domain: ArithmeticDomain) {
    use numerics::arithmetic::ArithmeticDomain;

    match domain {
        ArithmeticDomain::Exact => encoder.tag("exact", 0),
        ArithmeticDomain::Wrapping => encoder.tag("wrapping", 1),
        ArithmeticDomain::Saturating => encoder.tag("saturating", 2),
        ArithmeticDomain::Trapping => encoder.tag("trapping", 3),
    };
}
