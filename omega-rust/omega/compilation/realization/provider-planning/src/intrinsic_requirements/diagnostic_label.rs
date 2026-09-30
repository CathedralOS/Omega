//! Diagnostic labels for compiler-known intrinsic realizations.
//!
//! A label names the closed execution child, the named float operation or the
//! numeric conversion a requirement selects. `None` means the requirement has
//! no compiler-known migrated intrinsic, which plan validation and selected
//! dispatch both reject.

use super::IntrinsicRequirement;
use effects::CompilerIntrinsicExecutionIdentity;
use typed_trees::TypedTrees;

impl IntrinsicRequirement<'_> {
    /// Render the compiler-known realization this requirement selects. The
    /// label is diagnostic-only: provider identity and dispatch use the
    /// normalized realization-machine symbol retained in `ProviderBinding`.
    /// Keyed on the requirement view, a `boundary requirement` spelling of the
    /// same `Owner::name` signature renders identically to the operator.
    pub fn diagnostic_label(&self, typed: &TypedTrees) -> Option<String> {
        match self.closed_execution(typed) {
            Some(CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary {
                operation,
                format,
            }) => {
                return Some(format!("Float::{}.{}", operation.name(), format.name()));
            }
            Some(CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
                operation,
                integer_type,
            }) => {
                // No canonical requirement namespace exists for integer
                // comparisons; the authored token is the operation identity,
                // so the diagnostic label names the closed comparison shape
                // directly.
                return Some(format!(
                    "integer-comparison.{}.{}",
                    operation.name(),
                    integer_type.name()
                ));
            }
            _ => {}
        }
        let namespace = self.namespace.as_str();
        let requirement_name = self.name.as_str();
        let return_type = self.return_type;
        let parameters = self.parameters;
        let (operation, primitive, expected_result) = match namespace {
            "F32" | "F64" => {
                if matches!(requirement_name, "from_f64" | "from_f32") {
                    let (expected_source, expected_result, source_name) =
                        match (namespace, requirement_name) {
                            ("F32", "from_f64") => (
                                typed_trees::types::PrimitiveType::F64,
                                typed_trees::types::PrimitiveType::F32,
                                "f64",
                            ),
                            ("F64", "from_f32") => (
                                typed_trees::types::PrimitiveType::F32,
                                typed_trees::types::PrimitiveType::F64,
                                "f32",
                            ),
                            _ => return None,
                        };
                    let [value] = parameters else {
                        return None;
                    };
                    if typed.primitive_type_reference(value.type_reference) != Some(expected_source)
                        || typed.primitive_type_reference(return_type) != Some(expected_result)
                    {
                        return None;
                    }
                    return Some(format!("{}::{}.{source_name}", namespace, requirement_name));
                }
                if let Some(source_name) = requirement_name.strip_prefix("from_") {
                    let expected_source = match source_name {
                        "i8" => typed_trees::types::PrimitiveType::I8,
                        "i16" => typed_trees::types::PrimitiveType::I16,
                        "i32" => typed_trees::types::PrimitiveType::I32,
                        "i64" => typed_trees::types::PrimitiveType::I64,
                        "u8" => typed_trees::types::PrimitiveType::U8,
                        "u16" => typed_trees::types::PrimitiveType::U16,
                        "u32" => typed_trees::types::PrimitiveType::U32,
                        "u64" => typed_trees::types::PrimitiveType::U64,
                        _ => return None,
                    };
                    let expected_result = if namespace == "F32" {
                        typed_trees::types::PrimitiveType::F32
                    } else {
                        typed_trees::types::PrimitiveType::F64
                    };
                    let [value] = parameters else {
                        return None;
                    };
                    if typed.primitive_type_reference(value.type_reference) != Some(expected_source)
                        || typed.primitive_type_reference(return_type) != Some(expected_result)
                    {
                        return None;
                    }
                    return Some(format!("{}::{}.{source_name}", namespace, requirement_name));
                }
                let operation = match requirement_name {
                    "minimum" | "maximum" => requirement_name,
                    "negate"
                    | "square_root"
                    | "square_root_toward_zero"
                    | "square_root_toward_positive"
                    | "square_root_toward_negative"
                    | "classify"
                    | "is_nan"
                    | "is_finite"
                    | "is_infinite"
                    | "is_normal"
                    | "is_subnormal" => requirement_name,
                    "multiply_then_add"
                    | "fused_multiply_add"
                    | "fused_multiply_add_toward_zero"
                    | "fused_multiply_add_toward_positive"
                    | "fused_multiply_add_toward_negative" => requirement_name,
                    "add_toward_zero" | "add_toward_positive" | "add_toward_negative" => {
                        requirement_name
                    }
                    "subtract_toward_zero"
                    | "subtract_toward_positive"
                    | "subtract_toward_negative" => requirement_name,
                    "multiply_toward_zero"
                    | "multiply_toward_positive"
                    | "multiply_toward_negative" => requirement_name,
                    "divide_toward_zero" | "divide_toward_positive" | "divide_toward_negative" => {
                        requirement_name
                    }
                    _ => return None,
                };
                let expected_primitive = if namespace == "F32" {
                    typed_trees::types::PrimitiveType::F32
                } else {
                    typed_trees::types::PrimitiveType::F64
                };
                match parameters {
                    [value]
                        if matches!(
                            operation,
                            "negate"
                                | "square_root"
                                | "square_root_toward_zero"
                                | "square_root_toward_positive"
                                | "square_root_toward_negative"
                                | "classify"
                                | "is_nan"
                                | "is_finite"
                                | "is_infinite"
                                | "is_normal"
                                | "is_subnormal"
                        ) =>
                    {
                        if typed.primitive_type_reference(value.type_reference)
                            != Some(expected_primitive)
                        {
                            return None;
                        }
                    }
                    [left, right]
                        if matches!(
                            operation,
                            "minimum"
                                | "maximum"
                                | "add_toward_zero"
                                | "add_toward_positive"
                                | "add_toward_negative"
                                | "subtract_toward_zero"
                                | "subtract_toward_positive"
                                | "subtract_toward_negative"
                                | "multiply_toward_zero"
                                | "multiply_toward_positive"
                                | "multiply_toward_negative"
                                | "divide_toward_zero"
                                | "divide_toward_positive"
                                | "divide_toward_negative"
                        ) =>
                    {
                        if typed.primitive_type_reference(left.type_reference)
                            != Some(expected_primitive)
                            || typed.primitive_type_reference(right.type_reference)
                                != Some(expected_primitive)
                        {
                            return None;
                        }
                    }
                    [left, right, addend]
                        if matches!(
                            operation,
                            "multiply_then_add"
                                | "fused_multiply_add"
                                | "fused_multiply_add_toward_zero"
                                | "fused_multiply_add_toward_positive"
                                | "fused_multiply_add_toward_negative"
                        ) =>
                    {
                        if typed.primitive_type_reference(left.type_reference)
                            != Some(expected_primitive)
                            || typed.primitive_type_reference(right.type_reference)
                                != Some(expected_primitive)
                            || typed.primitive_type_reference(addend.type_reference)
                                != Some(expected_primitive)
                        {
                            return None;
                        }
                    }
                    _ => return None,
                }
                if operation == "classify" {
                    if typed.display_type_reference(return_type) != "FloatClass" {
                        return None;
                    }
                    let format = if expected_primitive == typed_trees::types::PrimitiveType::F32 {
                        "f32"
                    } else {
                        "f64"
                    };
                    return Some(format!("{}::classify.{format}", namespace));
                }
                let expected_result = if matches!(
                    operation,
                    "is_nan" | "is_finite" | "is_infinite" | "is_normal" | "is_subnormal"
                ) {
                    typed_trees::types::PrimitiveType::Bool
                } else {
                    expected_primitive
                };
                (operation, expected_primitive, expected_result)
            }
            "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64" => {
                let expected_result = match namespace {
                    "I8" => typed_trees::types::PrimitiveType::I8,
                    "I16" => typed_trees::types::PrimitiveType::I16,
                    "I32" => typed_trees::types::PrimitiveType::I32,
                    "I64" => typed_trees::types::PrimitiveType::I64,
                    "U8" => typed_trees::types::PrimitiveType::U8,
                    "U16" => typed_trees::types::PrimitiveType::U16,
                    "U32" => typed_trees::types::PrimitiveType::U32,
                    "U64" => typed_trees::types::PrimitiveType::U64,
                    _ => unreachable!(),
                };
                let (expected_source, source_name) = match requirement_name {
                    "from_f32" => (typed_trees::types::PrimitiveType::F32, "f32"),
                    "from_f64" => (typed_trees::types::PrimitiveType::F64, "f64"),
                    _ => return None,
                };
                let [value] = parameters else {
                    return None;
                };
                if typed.primitive_type_reference(value.type_reference) != Some(expected_source)
                    || typed.primitive_type_reference(return_type) != Some(expected_result)
                {
                    return None;
                }
                let policy = match typed.type_reference_table.arithmetic_domain(return_type) {
                    numerics::arithmetic::ArithmeticDomain::Exact => "exact",
                    numerics::arithmetic::ArithmeticDomain::Trapping => "trapping",
                    numerics::arithmetic::ArithmeticDomain::Saturating => "saturating",
                    numerics::arithmetic::ArithmeticDomain::Wrapping => return None,
                };
                return Some(format!(
                    "{}::{}.{source_name}.{policy}",
                    namespace, requirement_name
                ));
            }
            _ => return None,
        };
        if typed.primitive_type_reference(return_type) != Some(expected_result) {
            return None;
        }
        let format = match primitive {
            typed_trees::types::PrimitiveType::F32 => "f32",
            typed_trees::types::PrimitiveType::F64 => "f64",
            _ => return None,
        };
        Some(format!("{}::{operation}.{format}", namespace))
    }
}
