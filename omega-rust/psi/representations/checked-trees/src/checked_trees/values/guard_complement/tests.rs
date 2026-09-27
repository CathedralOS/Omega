use super::exact_complement;
use crate::values::{
    CheckedBooleanExpression as Boolean, CheckedIeeeFloatComparisonKind, CheckedScalarExpression,
    CheckedStructuralParameterField, CheckedStructuralPredicatePathSegment,
};
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

fn negate(expression: Boolean) -> Boolean {
    Boolean::Not(Box::new(expression))
}

fn equal_constant(expression: Boolean, value: bool) -> Boolean {
    Boolean::Equal {
        left: Box::new(expression),
        right: Box::new(Boolean::Constant(value)),
    }
}

fn no_cases(_: &CheckedStructuralParameterField) -> Option<Vec<String>> {
    None
}

fn scalar(position: usize, primitive_type: PrimitiveType) -> Box<CheckedScalarExpression> {
    Box::new(CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    })
}

fn float(kind: CheckedIeeeFloatComparisonKind, left: usize, right: usize) -> Boolean {
    Boolean::ScalarIeeeFloatComparison {
        kind,
        left: scalar(left, PrimitiveType::F64),
        right: scalar(right, PrimitiveType::F64),
    }
}

#[test]
fn polarity_pairs_preserve_exact_operands() {
    let parameter = Boolean::Parameter { position: 0 };
    let storage = Boolean::StorageRead {
        symbol: SymbolHandle::from_parts(2, 1),
    };
    for (first, second, accepted) in [
        (parameter.clone(), negate(parameter.clone()), true),
        (
            equal_constant(parameter.clone(), true),
            equal_constant(parameter.clone(), false),
            true,
        ),
        (
            equal_constant(parameter.clone(), false),
            parameter.clone(),
            true,
        ),
        (
            negate(equal_constant(negate(parameter.clone()), false)),
            parameter.clone(),
            true,
        ),
        (parameter.clone(), parameter.clone(), false),
        (
            parameter.clone(),
            negate(Boolean::Parameter { position: 1 }),
            false,
        ),
        (storage.clone(), negate(storage.clone()), true),
        (
            storage.clone(),
            negate(Boolean::StorageRead {
                symbol: SymbolHandle::from_parts(2, 2),
            }),
            false,
        ),
        (storage, negate(parameter), false),
    ] {
        assert_eq!(
            exact_complement(&first, &second, no_cases),
            accepted,
            "{first:?} / {second:?}"
        );
    }
}

#[test]
fn ieee_equality_pairs_are_complements_even_at_nan() {
    use CheckedIeeeFloatComparisonKind::{Equal, NotEqual};
    assert!(exact_complement(
        &float(Equal, 0, 1),
        &float(NotEqual, 0, 1),
        no_cases
    ));
    assert!(exact_complement(
        &float(NotEqual, 0, 1),
        &float(Equal, 0, 1),
        no_cases
    ));
    assert!(exact_complement(
        &float(Equal, 0, 1),
        &negate(float(Equal, 0, 1)),
        no_cases
    ));
    for (first, second) in [
        (float(Equal, 0, 1), float(Equal, 0, 1)),
        (float(Equal, 0, 1), float(NotEqual, 1, 0)),
        (float(Equal, 0, 1), negate(float(NotEqual, 0, 1))),
    ] {
        assert!(
            !exact_complement(&first, &second, no_cases),
            "{first:?} / {second:?}"
        );
    }
    let field = |identity: &str| CheckedStructuralParameterField {
        parameter_position: 0,
        path: vec![CheckedStructuralPredicatePathSegment::Field(
            identity.to_owned(),
        )],
    };
    let leaf = |kind, primitive_type| Boolean::IeeeFloatComparison {
        kind,
        primitive_type,
        left: field("x"),
        right: field("y"),
    };
    assert!(exact_complement(
        &leaf(Equal, PrimitiveType::F32),
        &leaf(NotEqual, PrimitiveType::F32),
        no_cases
    ));
    assert!(!exact_complement(
        &leaf(Equal, PrimitiveType::F32),
        &leaf(NotEqual, PrimitiveType::F64),
        no_cases
    ));
}
