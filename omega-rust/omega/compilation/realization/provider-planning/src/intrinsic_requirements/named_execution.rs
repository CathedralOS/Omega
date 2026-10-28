//! One numeric catalog row owns both its accepted signature and execution.
//!
//! This is classification of an already resolved requirement, not provider
//! selection or source authority. Callers still join the exact selected plan,
//! overload, package and external realization. Keeping these facts together
//! prevents a diagnostic-name recognizer and an execution-name recognizer from
//! accepting different operations.

use super::IntrinsicRequirement;
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::types::PrimitiveType;
use effects::CompilerIntrinsicExecutionIdentity;
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::FloatFormat;
use symbols::BuiltinFunction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NumericResult {
    Primitive(PrimitiveType),
    FloatClass,
}

#[derive(Clone, Copy)]
enum FloatResult {
    Float,
    Boolean,
    Class,
}

/// The finite native catalog, not a source-shape dispatch matrix. Each row
/// states one operation's arity, result and both existing IEEE realizations.
const FLOAT_OPERATIONS: &[(&str, usize, FloatResult, BuiltinFunction, BuiltinFunction)] = &[
    (
        "minimum",
        2,
        FloatResult::Float,
        BuiltinFunction::FloatMinimum,
        BuiltinFunction::FloatMinimum,
    ),
    (
        "maximum",
        2,
        FloatResult::Float,
        BuiltinFunction::FloatMaximum,
        BuiltinFunction::FloatMaximum,
    ),
    (
        "square_root",
        1,
        FloatResult::Float,
        BuiltinFunction::FloatSquareRoot,
        BuiltinFunction::FloatSquareRoot,
    ),
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
    (
        "is_nan",
        1,
        FloatResult::Boolean,
        BuiltinFunction::FloatIsNan,
        BuiltinFunction::FloatIsNan,
    ),
    (
        "is_finite",
        1,
        FloatResult::Boolean,
        BuiltinFunction::FloatIsFinite,
        BuiltinFunction::FloatIsFinite,
    ),
    (
        "is_infinite",
        1,
        FloatResult::Boolean,
        BuiltinFunction::FloatIsInfinite,
        BuiltinFunction::FloatIsInfinite,
    ),
    (
        "is_normal",
        1,
        FloatResult::Boolean,
        BuiltinFunction::FloatIsNormal,
        BuiltinFunction::FloatIsNormal,
    ),
    (
        "is_subnormal",
        1,
        FloatResult::Boolean,
        BuiltinFunction::FloatIsSubnormal,
        BuiltinFunction::FloatIsSubnormal,
    ),
    (
        "classify",
        1,
        FloatResult::Class,
        BuiltinFunction::FloatClassifyF32,
        BuiltinFunction::FloatClassifyF64,
    ),
    (
        "add_toward_zero",
        2,
        FloatResult::Float,
        BuiltinFunction::FloatAddTowardZeroF32,
        BuiltinFunction::FloatAddTowardZeroF64,
    ),
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

struct NamedNumericIntrinsic {
    operand: PrimitiveType,
    operand_count: usize,
    result: NumericResult,
    execution: CompilerIntrinsicExecutionIdentity,
}

impl NamedNumericIntrinsic {
    fn resolve(namespace: &str, name: &str, domain: ArithmeticDomain) -> Option<Self> {
        let target = numeric_primitive(namespace)?;
        if let Some(source_name) = name.strip_prefix("from_") {
            let source = conversion_operand(source_name)?;
            let source_numeric = super::primitive_numeric_type(source)?;
            let target_numeric = super::primitive_numeric_type(target)?;
            if (!source_numeric.is_float() && !target_numeric.is_float()) || source == target {
                return None;
            }
            let domain = if target_numeric.is_float() {
                ArithmeticDomain::Exact
            } else {
                match domain {
                    ArithmeticDomain::Exact
                    | ArithmeticDomain::Trapping
                    | ArithmeticDomain::Saturating => domain,
                    ArithmeticDomain::Wrapping => return None,
                }
            };
            return Some(Self {
                operand: source,
                operand_count: 1,
                result: NumericResult::Primitive(target),
                execution: CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
                    source: source_numeric,
                    target: target_numeric,
                    domain,
                },
            });
        }
        let format = match target {
            PrimitiveType::F32 => FloatFormat::F32,
            PrimitiveType::F64 => FloatFormat::F64,
            _ => return None,
        };
        if name == "negate" {
            return Some(Self {
                operand: target,
                operand_count: 1,
                result: NumericResult::Primitive(target),
                execution: CompilerIntrinsicExecutionIdentity::NamedFloatNegation(format),
            });
        }
        let (_, operand_count, result, f32_execution, f64_execution) = FLOAT_OPERATIONS
            .iter()
            .find(|(operation, ..)| *operation == name)?;
        let function = match format {
            FloatFormat::F32 => *f32_execution,
            FloatFormat::F64 => *f64_execution,
        };
        Some(Self {
            operand: target,
            operand_count: *operand_count,
            result: match result {
                FloatResult::Float => NumericResult::Primitive(target),
                FloatResult::Boolean => NumericResult::Primitive(PrimitiveType::Bool),
                FloatResult::Class => NumericResult::FloatClass,
            },
            execution: CompilerIntrinsicExecutionIdentity::BuiltinFunction(function),
        })
    }

    fn accepts(
        &self,
        parameters: impl ExactSizeIterator<Item = Option<PrimitiveType>>,
        result: Option<NumericResult>,
    ) -> bool {
        parameters.len() == self.operand_count
            && parameters
                .into_iter()
                .all(|parameter| parameter == Some(self.operand))
            && result == Some(self.result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use effects::CompilerNumericType;

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
                wrong_operand[0] = Some(PrimitiveType::I32);
                assert!(!row.accepts(wrong_operand.into_iter(), Some(row.result)));
                assert!(!row.accepts(
                    parameters.iter().copied(),
                    Some(NumericResult::Primitive(PrimitiveType::I32))
                ));
                assert!(!row.accepts(parameters.iter().copied(), None));
            }
        }
    }

    #[test]
    fn conversions_bind_both_carriers_and_the_result_domain() {
        for (namespace, name, domain, source, target, expected_domain) in [
            (
                "F32",
                "from_i64",
                ArithmeticDomain::Wrapping,
                CompilerNumericType::I64,
                CompilerNumericType::F32,
                ArithmeticDomain::Exact,
            ),
            (
                "F64",
                "from_f32",
                ArithmeticDomain::Exact,
                CompilerNumericType::F32,
                CompilerNumericType::F64,
                ArithmeticDomain::Exact,
            ),
            (
                "I32",
                "from_f64",
                ArithmeticDomain::Saturating,
                CompilerNumericType::F64,
                CompilerNumericType::I32,
                ArithmeticDomain::Saturating,
            ),
            (
                "U8",
                "from_f32",
                ArithmeticDomain::Trapping,
                CompilerNumericType::F32,
                CompilerNumericType::U8,
                ArithmeticDomain::Trapping,
            ),
        ] {
            let row = NamedNumericIntrinsic::resolve(namespace, name, domain).expect("conversion");
            assert_eq!(
                row.execution,
                CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
                    source,
                    target,
                    domain: expected_domain,
                }
            );
            assert!(row.accepts([Some(row.operand)].into_iter(), Some(row.result)));
            assert!(!row.accepts([Some(PrimitiveType::Bool)].into_iter(), Some(row.result)));
            assert!(!row.accepts(
                [Some(row.operand)].into_iter(),
                Some(NumericResult::Primitive(PrimitiveType::Bool))
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
            CompilerIntrinsicExecutionIdentity::NamedFloatNegation(FloatFormat::F32)
        );
        assert_eq!(
            resolve("negate").result,
            NumericResult::Primitive(PrimitiveType::F32)
        );
        assert_eq!(
            resolve("is_nan").result,
            NumericResult::Primitive(PrimitiveType::Bool)
        );
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
                CompilerIntrinsicExecutionIdentity::BuiltinFunction(function)
            );
        }
    }
}

fn numeric_primitive(namespace: &str) -> Option<PrimitiveType> {
    Some(match namespace {
        "F32" => PrimitiveType::F32,
        "F64" => PrimitiveType::F64,
        "I8" => PrimitiveType::I8,
        "I16" => PrimitiveType::I16,
        "I32" => PrimitiveType::I32,
        "I64" => PrimitiveType::I64,
        "U8" => PrimitiveType::U8,
        "U16" => PrimitiveType::U16,
        "U32" => PrimitiveType::U32,
        "U64" => PrimitiveType::U64,
        _ => return None,
    })
}

fn conversion_operand(name: &str) -> Option<PrimitiveType> {
    Some(match name {
        "f32" => PrimitiveType::F32,
        "f64" => PrimitiveType::F64,
        "i8" => PrimitiveType::I8,
        "i16" => PrimitiveType::I16,
        "i32" => PrimitiveType::I32,
        "i64" => PrimitiveType::I64,
        "u8" => PrimitiveType::U8,
        "u16" => PrimitiveType::U16,
        "u32" => PrimitiveType::U32,
        "u64" => PrimitiveType::U64,
        _ => return None,
    })
}

impl IntrinsicRequirement<'_> {
    pub(super) fn named_execution(
        &self,
        typed: &TypedTrees,
    ) -> Option<CompilerIntrinsicExecutionIdentity> {
        let intrinsic = NamedNumericIntrinsic::resolve(
            &self.namespace,
            &self.name,
            typed
                .type_reference_table
                .arithmetic_domain(self.return_type),
        )?;
        let result = typed
            .primitive_type_reference(self.return_type)
            .map(NumericResult::Primitive)
            .or_else(|| {
                (typed.display_type_reference(self.return_type) == "FloatClass")
                    .then_some(NumericResult::FloatClass)
            });
        intrinsic
            .accepts(
                self.parameters
                    .iter()
                    .map(|parameter| typed.primitive_type_reference(parameter.type_reference)),
                result,
            )
            .then_some(intrinsic.execution)
    }
}
