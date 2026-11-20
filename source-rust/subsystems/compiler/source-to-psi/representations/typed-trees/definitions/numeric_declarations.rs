//! Match a source declaration's namespace and spelling to its candidate numeric signature.
use numerics::arithmetic::ArithmeticDomain;
use numerics::integer_carrier::{FixedInteger, IntegerSign};
use numerics::intrinsics::{
    FloatBuiltin, FloatFormat, NumericIntrinsic, NumericSignature, NumericType,
};

pub(super) fn resolve(
    namespace: &str,
    name: &str,
    domain: ArithmeticDomain,
) -> Option<NumericSignature> {
    let target = numeric_primitive(namespace)?;
    let meaning = if let Some(source_name) = name.strip_prefix("from_") {
        let source = conversion_operand(source_name)?;
        NumericIntrinsic::Conversion {
            source,
            target,
            domain: if target.is_float() {
                ArithmeticDomain::Exact
            } else {
                domain
            },
        }
    } else {
        let format = match target {
            NumericType::F32 => FloatFormat::F32,
            NumericType::F64 => FloatFormat::F64,
            _ => return None,
        };
        if name == "negate" {
            NumericIntrinsic::Negation(format)
        } else {
            float_builtin(name)?.intrinsic(format)
        }
    };
    meaning.signature()
}

fn numeric_primitive(namespace: &str) -> Option<NumericType> {
    match namespace {
        "F32" => return Some(NumericType::F32),
        "F64" => return Some(NumericType::F64),
        _ => {}
    }
    let (sign, digits) = if let Some(digits) = namespace.strip_prefix('I') {
        (IntegerSign::Signed, digits)
    } else {
        (IntegerSign::Unsigned, namespace.strip_prefix('U')?)
    };
    FixedInteger::from_decimal_digits(sign, digits).map(NumericType::Integer)
}

fn conversion_operand(name: &str) -> Option<NumericType> {
    Some(match name {
        "f32" => NumericType::F32,
        "f64" => NumericType::F64,
        _ => NumericType::Integer(FixedInteger::from_decimal_spelling(name)?),
    })
}

const FLOAT_BUILTINS: &[(&str, FloatBuiltin)] = &[
    ("minimum", FloatBuiltin::MINIMUM),
    ("maximum", FloatBuiltin::MAXIMUM),
    ("square_root", FloatBuiltin::SQUARE_ROOT),
    ("multiply_then_add", FloatBuiltin::MULTIPLY_THEN_ADD),
    ("fused_multiply_add", FloatBuiltin::FUSED_MULTIPLY_ADD),
    ("is_nan", FloatBuiltin::IS_NAN),
    ("is_finite", FloatBuiltin::IS_FINITE),
    ("is_infinite", FloatBuiltin::IS_INFINITE),
    ("is_normal", FloatBuiltin::IS_NORMAL),
    ("is_subnormal", FloatBuiltin::IS_SUBNORMAL),
    ("classify", FloatBuiltin::CLASSIFY),
    ("add_toward_zero", FloatBuiltin::ADD_TOWARD_ZERO),
    ("add_toward_positive", FloatBuiltin::ADD_TOWARD_POSITIVE),
    ("add_toward_negative", FloatBuiltin::ADD_TOWARD_NEGATIVE),
    ("subtract_toward_zero", FloatBuiltin::SUBTRACT_TOWARD_ZERO),
    (
        "subtract_toward_positive",
        FloatBuiltin::SUBTRACT_TOWARD_POSITIVE,
    ),
    (
        "subtract_toward_negative",
        FloatBuiltin::SUBTRACT_TOWARD_NEGATIVE,
    ),
    ("multiply_toward_zero", FloatBuiltin::MULTIPLY_TOWARD_ZERO),
    (
        "multiply_toward_positive",
        FloatBuiltin::MULTIPLY_TOWARD_POSITIVE,
    ),
    (
        "multiply_toward_negative",
        FloatBuiltin::MULTIPLY_TOWARD_NEGATIVE,
    ),
    ("divide_toward_zero", FloatBuiltin::DIVIDE_TOWARD_ZERO),
    (
        "divide_toward_positive",
        FloatBuiltin::DIVIDE_TOWARD_POSITIVE,
    ),
    (
        "divide_toward_negative",
        FloatBuiltin::DIVIDE_TOWARD_NEGATIVE,
    ),
    (
        "square_root_toward_zero",
        FloatBuiltin::SQUARE_ROOT_TOWARD_ZERO,
    ),
    (
        "square_root_toward_positive",
        FloatBuiltin::SQUARE_ROOT_TOWARD_POSITIVE,
    ),
    (
        "square_root_toward_negative",
        FloatBuiltin::SQUARE_ROOT_TOWARD_NEGATIVE,
    ),
    (
        "fused_multiply_add_toward_zero",
        FloatBuiltin::FUSED_MULTIPLY_ADD_TOWARD_ZERO,
    ),
    (
        "fused_multiply_add_toward_positive",
        FloatBuiltin::FUSED_MULTIPLY_ADD_TOWARD_POSITIVE,
    ),
    (
        "fused_multiply_add_toward_negative",
        FloatBuiltin::FUSED_MULTIPLY_ADD_TOWARD_NEGATIVE,
    ),
];

fn float_builtin(name: &str) -> Option<FloatBuiltin> {
    FLOAT_BUILTINS
        .iter()
        .find_map(|(spelling, builtin)| (*spelling == name).then_some(*builtin))
}

#[cfg(test)]
mod tests {
    use super::*;
    use language_core::BuiltinFunction;
    use numerics::intrinsics::NumericResult;

    #[test]
    fn every_named_float_row_requires_its_complete_signature() {
        for (row_index, (name, builtin)) in FLOAT_BUILTINS.iter().enumerate() {
            let arity = builtin
                .intrinsic(FloatFormat::F32)
                .signature()
                .unwrap()
                .operand_count;
            assert!(!FLOAT_BUILTINS[..row_index].iter().any(|row| row.0 == *name));
            assert_eq!(
                numerics::float_semantics_catalog::FloatSemanticOperation::named_call_arity(name),
                Some(arity),
                "{name}",
            );
            for namespace in ["F32", "F64"] {
                let row = resolve(namespace, name, ArithmeticDomain::Exact)
                    .expect("catalog operation resolves");
                let bytes = row
                    .execution
                    .canonical_encoding()
                    .expect("closed wire meaning");
                assert_eq!(
                    NumericIntrinsic::from_canonical_encoding(bytes),
                    Some(row.execution)
                );
                let parameters = vec![Some(row.operand); arity];
                assert!(
                    row.accepts(parameters.iter().copied(), Some(row.result)),
                    "{namespace}::{name}"
                );
                assert!(!row.accepts(parameters[..arity - 1].iter().copied(), Some(row.result)));
                let mut excess = parameters.clone();
                excess.push(Some(row.operand));
                assert!(!row.accepts(excess.into_iter(), Some(row.result)));
                let mut wrong_operand = parameters.clone();
                wrong_operand[0] = Some(NumericType::Integer(FixedInteger::signed(32)));
                assert!(!row.accepts(wrong_operand.into_iter(), Some(row.result)));
                assert!(!row.accepts(
                    parameters.iter().copied(),
                    Some(NumericResult::Primitive(NumericType::Integer(
                        FixedInteger::signed(32)
                    )))
                ));
                assert!(!row.accepts(parameters.iter().copied(), None));
            }
        }
    }

    #[test]
    fn unusual_width_numeric_intrinsics_retain_exact_conversion_meaning() {
        for (namespace, name) in [("F32", "from_u13"), ("I257", "from_f64")] {
            let row = resolve(namespace, name, ArithmeticDomain::Exact)
                .expect("numeric signatures retain arbitrary integer widths");
            assert_eq!(
                NumericIntrinsic::from_canonical_encoding(
                    row.execution.canonical_encoding().unwrap()
                ),
                Some(row.execution)
            );
            assert!(row.accepts([Some(row.operand)].into_iter(), Some(row.result)));
            assert!(!row.accepts([None].into_iter(), Some(row.result)));
        }
    }

    #[test]
    fn conversions_bind_both_carriers_and_the_result_domain() {
        for (namespace, name, domain, source, target, expected_domain) in [
            (
                "F32",
                "from_i64",
                ArithmeticDomain::Wrapping,
                NumericType::Integer(FixedInteger::signed(64)),
                NumericType::F32,
                ArithmeticDomain::Exact,
            ),
            (
                "F64",
                "from_f32",
                ArithmeticDomain::Exact,
                NumericType::F32,
                NumericType::F64,
                ArithmeticDomain::Exact,
            ),
            (
                "I32",
                "from_f64",
                ArithmeticDomain::Saturating,
                NumericType::F64,
                NumericType::Integer(FixedInteger::signed(32)),
                ArithmeticDomain::Saturating,
            ),
            (
                "U8",
                "from_f32",
                ArithmeticDomain::Trapping,
                NumericType::F32,
                NumericType::Integer(FixedInteger::unsigned(8)),
                ArithmeticDomain::Trapping,
            ),
        ] {
            let row = resolve(namespace, name, domain).expect("conversion");
            assert_eq!(
                NumericIntrinsic::from_canonical_encoding(
                    row.execution.canonical_encoding().unwrap()
                ),
                Some(row.execution)
            );
            assert_eq!(
                row.execution,
                NumericIntrinsic::Conversion {
                    source,
                    target,
                    domain: expected_domain,
                }
            );
            assert!(row.accepts([Some(row.operand)].into_iter(), Some(row.result)));
            assert!(!row.accepts([None].into_iter(), Some(row.result)));
            assert!(!row.accepts(
                [Some(row.operand)].into_iter(),
                Some(NumericResult::Boolean)
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
                resolve(namespace, name, domain).is_none(),
                "{namespace}::{name}"
            );
        }
    }

    #[test]
    fn result_categories_and_rounding_are_distinct_execution_meanings() {
        let resolve = |name| resolve("F32", name, ArithmeticDomain::Exact).expect("operation");
        assert_eq!(
            resolve("negate").execution,
            NumericIntrinsic::Negation(FloatFormat::F32)
        );
        assert_eq!(
            resolve("negate").result,
            NumericResult::Primitive(NumericType::F32)
        );
        assert_eq!(resolve("is_nan").result, NumericResult::Boolean);
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
                NumericIntrinsic::Builtin {
                    function,
                    format: FloatFormat::F32
                }
            );
        }
    }
}
