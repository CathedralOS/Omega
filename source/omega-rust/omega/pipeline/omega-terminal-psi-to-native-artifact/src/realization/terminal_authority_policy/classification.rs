//! Optimizer module role: classification leaf. Closed intrinsic authority dispositions.

use omega_effects::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    TerminalAuthorityClass, TerminalAuthorityDisposition,
};
use psi_numerics::{arithmetic::ArithmeticDomain, literals::FloatFormat};
use psi_symbols::BuiltinFunction;

use super::model::UnclassifiedTerminalMechanism;

pub(super) fn classify_compiler_intrinsic(
    mechanism: CompilerIntrinsicExecutionIdentity,
) -> TerminalAuthorityDisposition {
    match mechanism {
        CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32 => {
            disposition([TerminalAuthorityClass::ProcessTermination])
        }
        CompilerIntrinsicExecutionIdentity::LinuxWriteByteI32 => {
            disposition([TerminalAuthorityClass::ProcessOutput])
        }
        CompilerIntrinsicExecutionIdentity::LinuxReadByte => {
            disposition([TerminalAuthorityClass::ProcessInput])
        }
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(function) => {
            classify_builtin_function(function)
        }
        CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format } => {
            classify_primitive_float_binary(operation, format)
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatNegation(format) => {
            classify_named_float_negation(format)
        }
        CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
            source,
            target,
            domain,
        } => classify_named_float_conversion(source, target, domain),
    }
}

pub(super) fn classify_from_inventory(
    inventory: &[CompilerIntrinsicExecutionIdentity],
    mechanism: CompilerIntrinsicExecutionIdentity,
) -> Result<TerminalAuthorityDisposition, UnclassifiedTerminalMechanism> {
    if !inventory.contains(&mechanism) {
        return Err(UnclassifiedTerminalMechanism {
            mechanism: mechanism.into(),
        });
    }
    Ok(classify_compiler_intrinsic(mechanism))
}

fn classify_primitive_float_binary(
    operation: CompilerPrimitiveFloatBinaryOperation,
    format: FloatFormat,
) -> TerminalAuthorityDisposition {
    let _closed_coordinates = (
        primitive_float_operation_tag(operation),
        float_format_tag(format),
    );
    empty_disposition()
}

fn classify_named_float_negation(format: FloatFormat) -> TerminalAuthorityDisposition {
    let _closed_format = float_format_tag(format);
    empty_disposition()
}

fn classify_named_float_conversion(
    source: CompilerNumericType,
    target: CompilerNumericType,
    domain: ArithmeticDomain,
) -> TerminalAuthorityDisposition {
    let _closed_coordinates = (
        numeric_type_tag(source),
        numeric_type_tag(target),
        arithmetic_domain_tag(domain),
    );
    empty_disposition()
}

fn classify_builtin_function(function: BuiltinFunction) -> TerminalAuthorityDisposition {
    match function {
        BuiltinFunction::AsmHlt
        | BuiltinFunction::AsmDisableInterrupts
        | BuiltinFunction::AsmEnableInterrupts
        | BuiltinFunction::AsmRestoreFlags
        | BuiltinFunction::AsmReadMsr
        | BuiltinFunction::AsmWriteMsr
        | BuiltinFunction::AsmReadCr0
        | BuiltinFunction::AsmReadCr2
        | BuiltinFunction::AsmReadCr3
        | BuiltinFunction::AsmReadCr4
        | BuiltinFunction::AsmWriteCr0
        | BuiltinFunction::AsmWriteCr3
        | BuiltinFunction::AsmWriteCr4 => disposition([TerminalAuthorityClass::MachineControl]),
        BuiltinFunction::AsmPortOut | BuiltinFunction::AsmPortIn => {
            disposition([TerminalAuthorityClass::PortIo])
        }
        BuiltinFunction::Max
        | BuiltinFunction::Min
        | BuiltinFunction::Sqrt
        | BuiltinFunction::AsmLoadFence
        | BuiltinFunction::AsmStoreFence
        | BuiltinFunction::AsmFullFence
        | BuiltinFunction::AsmSnapshotFlags
        | BuiltinFunction::FloatIsNan
        | BuiltinFunction::FloatMultiplyThenAddF32
        | BuiltinFunction::FloatMultiplyThenAddF64
        | BuiltinFunction::FloatFusedMultiplyAddF32
        | BuiltinFunction::FloatFusedMultiplyAddF64
        | BuiltinFunction::FloatIsFinite
        | BuiltinFunction::FloatIsInfinite
        | BuiltinFunction::FloatIsNormal
        | BuiltinFunction::FloatIsSubnormal
        | BuiltinFunction::FloatClassifyF32
        | BuiltinFunction::FloatClassifyF64
        | BuiltinFunction::ContentOld
        | BuiltinFunction::ContentSeparate
        | BuiltinFunction::FloatAddTowardZeroF32
        | BuiltinFunction::FloatAddTowardZeroF64
        | BuiltinFunction::FloatAddTowardPositiveF32
        | BuiltinFunction::FloatAddTowardPositiveF64
        | BuiltinFunction::FloatAddTowardNegativeF32
        | BuiltinFunction::FloatAddTowardNegativeF64
        | BuiltinFunction::FloatSubtractTowardZeroF32
        | BuiltinFunction::FloatSubtractTowardZeroF64
        | BuiltinFunction::FloatSubtractTowardPositiveF32
        | BuiltinFunction::FloatSubtractTowardPositiveF64
        | BuiltinFunction::FloatSubtractTowardNegativeF32
        | BuiltinFunction::FloatSubtractTowardNegativeF64
        | BuiltinFunction::FloatMultiplyTowardZeroF32
        | BuiltinFunction::FloatMultiplyTowardZeroF64
        | BuiltinFunction::FloatMultiplyTowardPositiveF32
        | BuiltinFunction::FloatMultiplyTowardPositiveF64
        | BuiltinFunction::FloatMultiplyTowardNegativeF32
        | BuiltinFunction::FloatMultiplyTowardNegativeF64
        | BuiltinFunction::FloatDivideTowardZeroF32
        | BuiltinFunction::FloatDivideTowardZeroF64
        | BuiltinFunction::FloatDivideTowardPositiveF32
        | BuiltinFunction::FloatDivideTowardPositiveF64
        | BuiltinFunction::FloatDivideTowardNegativeF32
        | BuiltinFunction::FloatDivideTowardNegativeF64
        | BuiltinFunction::FloatSqrtTowardZeroF32
        | BuiltinFunction::FloatSqrtTowardZeroF64
        | BuiltinFunction::FloatSqrtTowardPositiveF32
        | BuiltinFunction::FloatSqrtTowardPositiveF64
        | BuiltinFunction::FloatSqrtTowardNegativeF32
        | BuiltinFunction::FloatSqrtTowardNegativeF64
        | BuiltinFunction::FloatFusedMultiplyAddTowardZeroF32
        | BuiltinFunction::FloatFusedMultiplyAddTowardZeroF64
        | BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF32
        | BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF64
        | BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF32
        | BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF64 => empty_disposition(),
    }
}

fn disposition<const N: usize>(
    classes: [TerminalAuthorityClass; N],
) -> TerminalAuthorityDisposition {
    TerminalAuthorityDisposition::from_classes(classes)
}

fn empty_disposition() -> TerminalAuthorityDisposition {
    disposition([])
}

const fn primitive_float_operation_tag(operation: CompilerPrimitiveFloatBinaryOperation) -> u8 {
    match operation {
        CompilerPrimitiveFloatBinaryOperation::Add => 0,
        CompilerPrimitiveFloatBinaryOperation::Subtract => 1,
        CompilerPrimitiveFloatBinaryOperation::Multiply => 2,
        CompilerPrimitiveFloatBinaryOperation::Divide => 3,
        CompilerPrimitiveFloatBinaryOperation::Equal => 4,
        CompilerPrimitiveFloatBinaryOperation::NotEqual => 5,
        CompilerPrimitiveFloatBinaryOperation::Less => 6,
        CompilerPrimitiveFloatBinaryOperation::LessOrEqual => 7,
        CompilerPrimitiveFloatBinaryOperation::Greater => 8,
        CompilerPrimitiveFloatBinaryOperation::GreaterOrEqual => 9,
    }
}

const fn numeric_type_tag(numeric_type: CompilerNumericType) -> u8 {
    match numeric_type {
        CompilerNumericType::I8 => 0,
        CompilerNumericType::I16 => 1,
        CompilerNumericType::I32 => 2,
        CompilerNumericType::I64 => 3,
        CompilerNumericType::U8 => 4,
        CompilerNumericType::U16 => 5,
        CompilerNumericType::U32 => 6,
        CompilerNumericType::U64 => 7,
        CompilerNumericType::F32 => 8,
        CompilerNumericType::F64 => 9,
    }
}

const fn float_format_tag(format: FloatFormat) -> u8 {
    match format {
        FloatFormat::F32 => 0,
        FloatFormat::F64 => 1,
    }
}

const fn arithmetic_domain_tag(domain: ArithmeticDomain) -> u8 {
    match domain {
        ArithmeticDomain::Exact => 0,
        ArithmeticDomain::Wrapping => 1,
        ArithmeticDomain::Saturating => 2,
        ArithmeticDomain::Trapping => 3,
    }
}
