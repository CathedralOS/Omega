//! Named and directed float realizations of builtins and operators.

use crate::selected_dispatch::float_intrinsic::{
    DirectedFloatBinaryOperation, NamedFloatRealization, StagedNamedFloatExecution,
};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use numerics::float_semantics::RoundingDirection;
use numerics::literals::FloatFormat;
use provider_planning::IntrinsicRequirement;
use symbols::BuiltinFunction;

pub(crate) fn preflight_named_float_execution(
    checked: &CheckedTrees,
    requirement: &IntrinsicRequirement<'_>,
    realization: NamedFloatRealization,
) -> Result<StagedNamedFloatExecution, Diagnostic> {
    if let NamedFloatRealization::Negate(format) = realization {
        return Ok(StagedNamedFloatExecution::Negate(format));
    }
    if let NamedFloatRealization::Convert(domain) = realization {
        if !requirement.return_type.is_valid() {
            return Err(Diagnostic::error(
                "selected named conversion intrinsic has no exact return type",
            ));
        }
        return Ok(StagedNamedFloatExecution::Convert {
            domain,
            target_type: requirement.return_type,
        });
    }
    let function = named_float_realization_builtin(realization)?;
    let symbol = checked
        .typed
        .symbols
        .builtin_function_symbol(function)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "compiler builtin `{}` is absent while preflighting a selected named float intrinsic",
                function.name(),
            ))
        })?;
    Ok(StagedNamedFloatExecution::Builtin { function, symbol })
}

pub(crate) fn named_float_realization_builtin(
    realization: NamedFloatRealization,
) -> Result<BuiltinFunction, Diagnostic> {
    let function = match realization {
        NamedFloatRealization::Builtin { function, .. } => function,
        // Keep multiply-then-add and FMA as distinct unnameable builtins.
        // The first executes two explicit roundings; the second remains one
        // fused operation through instruction selection.
        NamedFloatRealization::MultiplyThenAdd(FloatFormat::F32) => {
            BuiltinFunction::FloatMultiplyThenAddF32
        }
        NamedFloatRealization::MultiplyThenAdd(FloatFormat::F64) => {
            BuiltinFunction::FloatMultiplyThenAddF64
        }
        NamedFloatRealization::FusedMultiplyAdd(FloatFormat::F32) => {
            BuiltinFunction::FloatFusedMultiplyAddF32
        }
        NamedFloatRealization::FusedMultiplyAdd(FloatFormat::F64) => {
            BuiltinFunction::FloatFusedMultiplyAddF64
        }
        NamedFloatRealization::DirectedFusedMultiplyAdd(format, direction) => {
            directed_fused_multiply_add_builtin(format, direction)?
        }
        NamedFloatRealization::DirectedSquareRoot(format, direction) => {
            directed_square_root_builtin(format, direction)?
        }
        NamedFloatRealization::DirectedBinary(operation, format, direction) => {
            directed_binary_builtin(operation, format, direction)?
        }
        NamedFloatRealization::Negate(_) | NamedFloatRealization::Convert(_) => {
            return Err(Diagnostic::error(
                "non-builtin named-float realization reached builtin preflight",
            ));
        }
    };
    Ok(function)
}

/// Resolve execution from the exact checked operator shape. The retained
/// catalog label is diagnostic-only; it is never parsed or used as a dispatch
/// key.
/// The compiler-known named float realization of one `Owner::name`
/// requirement, keyed on the requirement view so a `boundary operator` and a
/// `boundary requirement` spelling of the same signature select the same
/// realization.
pub(crate) fn named_float_realization_for(
    typed: &typed_trees::TypedTrees,
    requirement: &IntrinsicRequirement<'_>,
) -> Option<NamedFloatRealization> {
    let namespace = requirement.namespace.as_str();
    let return_type = requirement.return_type;
    let requirement = requirement.name.as_str();
    if matches!(
        namespace,
        "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64"
    ) {
        return matches!(requirement, "from_f32" | "from_f64").then(|| {
            NamedFloatRealization::Convert(
                typed.type_reference_table.arithmetic_domain(return_type),
            )
        });
    }
    let format = match namespace {
        "F32" => FloatFormat::F32,
        "F64" => FloatFormat::F64,
        _ => return None,
    };
    if requirement.starts_with("from_") {
        return Some(NamedFloatRealization::Convert(ArithmeticDomain::Exact));
    }
    let builtin = |function, arity| NamedFloatRealization::Builtin { function, arity };
    match requirement {
        "minimum" => Some(builtin(BuiltinFunction::Min, 2)),
        "maximum" => Some(builtin(BuiltinFunction::Max, 2)),
        "square_root" => Some(builtin(BuiltinFunction::Sqrt, 1)),
        "negate" => Some(NamedFloatRealization::Negate(format)),
        "multiply_then_add" => Some(NamedFloatRealization::MultiplyThenAdd(format)),
        "fused_multiply_add" => Some(NamedFloatRealization::FusedMultiplyAdd(format)),
        "is_nan" => Some(builtin(BuiltinFunction::FloatIsNan, 1)),
        "is_finite" => Some(builtin(BuiltinFunction::FloatIsFinite, 1)),
        "is_infinite" => Some(builtin(BuiltinFunction::FloatIsInfinite, 1)),
        "is_normal" => Some(builtin(BuiltinFunction::FloatIsNormal, 1)),
        "is_subnormal" => Some(builtin(BuiltinFunction::FloatIsSubnormal, 1)),
        "classify" => Some(builtin(
            match format {
                FloatFormat::F32 => BuiltinFunction::FloatClassifyF32,
                FloatFormat::F64 => BuiltinFunction::FloatClassifyF64,
            },
            1,
        )),
        _ => directed_float_realization(requirement, format),
    }
}

fn directed_float_realization(
    requirement: &str,
    format: FloatFormat,
) -> Option<NamedFloatRealization> {
    for (suffix, direction) in [
        ("_toward_zero", RoundingDirection::TowardZero),
        ("_toward_positive", RoundingDirection::TowardPositive),
        ("_toward_negative", RoundingDirection::TowardNegative),
    ] {
        let Some(operation) = requirement.strip_suffix(suffix) else {
            continue;
        };
        return match operation {
            "add" => Some(NamedFloatRealization::DirectedBinary(
                DirectedFloatBinaryOperation::Add,
                format,
                direction,
            )),
            "subtract" => Some(NamedFloatRealization::DirectedBinary(
                DirectedFloatBinaryOperation::Subtract,
                format,
                direction,
            )),
            "multiply" => Some(NamedFloatRealization::DirectedBinary(
                DirectedFloatBinaryOperation::Multiply,
                format,
                direction,
            )),
            "divide" => Some(NamedFloatRealization::DirectedBinary(
                DirectedFloatBinaryOperation::Divide,
                format,
                direction,
            )),
            "square_root" => Some(NamedFloatRealization::DirectedSquareRoot(format, direction)),
            "fused_multiply_add" => Some(NamedFloatRealization::DirectedFusedMultiplyAdd(
                format, direction,
            )),
            _ => None,
        };
    }
    None
}

fn directed_fused_multiply_add_builtin(
    format: FloatFormat,
    direction: RoundingDirection,
) -> Result<BuiltinFunction, Diagnostic> {
    match (format, direction) {
        (FloatFormat::F32, RoundingDirection::TowardZero) => {
            Ok(BuiltinFunction::FloatFusedMultiplyAddTowardZeroF32)
        }
        (FloatFormat::F64, RoundingDirection::TowardZero) => {
            Ok(BuiltinFunction::FloatFusedMultiplyAddTowardZeroF64)
        }
        (FloatFormat::F32, RoundingDirection::TowardPositive) => {
            Ok(BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF32)
        }
        (FloatFormat::F64, RoundingDirection::TowardPositive) => {
            Ok(BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF64)
        }
        (FloatFormat::F32, RoundingDirection::TowardNegative) => {
            Ok(BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF32)
        }
        (FloatFormat::F64, RoundingDirection::TowardNegative) => {
            Ok(BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF64)
        }
        (_, RoundingDirection::NearestTiesToEven) => Err(Diagnostic::error(
            "directed FMA realization cannot select nearest-even",
        )),
    }
}

fn directed_square_root_builtin(
    format: FloatFormat,
    direction: RoundingDirection,
) -> Result<BuiltinFunction, Diagnostic> {
    match (format, direction) {
        (FloatFormat::F32, RoundingDirection::TowardZero) => {
            Ok(BuiltinFunction::FloatSqrtTowardZeroF32)
        }
        (FloatFormat::F64, RoundingDirection::TowardZero) => {
            Ok(BuiltinFunction::FloatSqrtTowardZeroF64)
        }
        (FloatFormat::F32, RoundingDirection::TowardPositive) => {
            Ok(BuiltinFunction::FloatSqrtTowardPositiveF32)
        }
        (FloatFormat::F64, RoundingDirection::TowardPositive) => {
            Ok(BuiltinFunction::FloatSqrtTowardPositiveF64)
        }
        (FloatFormat::F32, RoundingDirection::TowardNegative) => {
            Ok(BuiltinFunction::FloatSqrtTowardNegativeF32)
        }
        (FloatFormat::F64, RoundingDirection::TowardNegative) => {
            Ok(BuiltinFunction::FloatSqrtTowardNegativeF64)
        }
        (_, RoundingDirection::NearestTiesToEven) => Err(Diagnostic::error(
            "directed square-root realization cannot select nearest-even",
        )),
    }
}

fn directed_binary_builtin(
    operation: DirectedFloatBinaryOperation,
    format: FloatFormat,
    direction: RoundingDirection,
) -> Result<BuiltinFunction, Diagnostic> {
    let function = match (operation, format, direction) {
        (DirectedFloatBinaryOperation::Add, FloatFormat::F32, RoundingDirection::TowardZero) => {
            BuiltinFunction::FloatAddTowardZeroF32
        }
        (DirectedFloatBinaryOperation::Add, FloatFormat::F64, RoundingDirection::TowardZero) => {
            BuiltinFunction::FloatAddTowardZeroF64
        }
        (
            DirectedFloatBinaryOperation::Add,
            FloatFormat::F32,
            RoundingDirection::TowardPositive,
        ) => BuiltinFunction::FloatAddTowardPositiveF32,
        (
            DirectedFloatBinaryOperation::Add,
            FloatFormat::F64,
            RoundingDirection::TowardPositive,
        ) => BuiltinFunction::FloatAddTowardPositiveF64,
        (
            DirectedFloatBinaryOperation::Add,
            FloatFormat::F32,
            RoundingDirection::TowardNegative,
        ) => BuiltinFunction::FloatAddTowardNegativeF32,
        (
            DirectedFloatBinaryOperation::Add,
            FloatFormat::F64,
            RoundingDirection::TowardNegative,
        ) => BuiltinFunction::FloatAddTowardNegativeF64,
        (
            DirectedFloatBinaryOperation::Subtract,
            FloatFormat::F32,
            RoundingDirection::TowardZero,
        ) => BuiltinFunction::FloatSubtractTowardZeroF32,
        (
            DirectedFloatBinaryOperation::Subtract,
            FloatFormat::F64,
            RoundingDirection::TowardZero,
        ) => BuiltinFunction::FloatSubtractTowardZeroF64,
        (
            DirectedFloatBinaryOperation::Subtract,
            FloatFormat::F32,
            RoundingDirection::TowardPositive,
        ) => BuiltinFunction::FloatSubtractTowardPositiveF32,
        (
            DirectedFloatBinaryOperation::Subtract,
            FloatFormat::F64,
            RoundingDirection::TowardPositive,
        ) => BuiltinFunction::FloatSubtractTowardPositiveF64,
        (
            DirectedFloatBinaryOperation::Subtract,
            FloatFormat::F32,
            RoundingDirection::TowardNegative,
        ) => BuiltinFunction::FloatSubtractTowardNegativeF32,
        (
            DirectedFloatBinaryOperation::Subtract,
            FloatFormat::F64,
            RoundingDirection::TowardNegative,
        ) => BuiltinFunction::FloatSubtractTowardNegativeF64,
        (
            DirectedFloatBinaryOperation::Multiply,
            FloatFormat::F32,
            RoundingDirection::TowardZero,
        ) => BuiltinFunction::FloatMultiplyTowardZeroF32,
        (
            DirectedFloatBinaryOperation::Multiply,
            FloatFormat::F64,
            RoundingDirection::TowardZero,
        ) => BuiltinFunction::FloatMultiplyTowardZeroF64,
        (
            DirectedFloatBinaryOperation::Multiply,
            FloatFormat::F32,
            RoundingDirection::TowardPositive,
        ) => BuiltinFunction::FloatMultiplyTowardPositiveF32,
        (
            DirectedFloatBinaryOperation::Multiply,
            FloatFormat::F64,
            RoundingDirection::TowardPositive,
        ) => BuiltinFunction::FloatMultiplyTowardPositiveF64,
        (
            DirectedFloatBinaryOperation::Multiply,
            FloatFormat::F32,
            RoundingDirection::TowardNegative,
        ) => BuiltinFunction::FloatMultiplyTowardNegativeF32,
        (
            DirectedFloatBinaryOperation::Multiply,
            FloatFormat::F64,
            RoundingDirection::TowardNegative,
        ) => BuiltinFunction::FloatMultiplyTowardNegativeF64,
        (DirectedFloatBinaryOperation::Divide, FloatFormat::F32, RoundingDirection::TowardZero) => {
            BuiltinFunction::FloatDivideTowardZeroF32
        }
        (DirectedFloatBinaryOperation::Divide, FloatFormat::F64, RoundingDirection::TowardZero) => {
            BuiltinFunction::FloatDivideTowardZeroF64
        }
        (
            DirectedFloatBinaryOperation::Divide,
            FloatFormat::F32,
            RoundingDirection::TowardPositive,
        ) => BuiltinFunction::FloatDivideTowardPositiveF32,
        (
            DirectedFloatBinaryOperation::Divide,
            FloatFormat::F64,
            RoundingDirection::TowardPositive,
        ) => BuiltinFunction::FloatDivideTowardPositiveF64,
        (
            DirectedFloatBinaryOperation::Divide,
            FloatFormat::F32,
            RoundingDirection::TowardNegative,
        ) => BuiltinFunction::FloatDivideTowardNegativeF32,
        (
            DirectedFloatBinaryOperation::Divide,
            FloatFormat::F64,
            RoundingDirection::TowardNegative,
        ) => BuiltinFunction::FloatDivideTowardNegativeF64,
        (_, _, RoundingDirection::NearestTiesToEven) => {
            return Err(Diagnostic::error(
                "directed float realization cannot select nearest-even",
            ));
        }
    };
    Ok(function)
}
