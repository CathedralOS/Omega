//! Closed scalar contract predicates carry exact integer arithmetic over
//! entry/result subjects and contextual literals; a conjunction of such
//! equalities lowers as one predicate, while domain-qualified operands and
//! non-arithmetic shapes stay outside the closed language.

use crate::tests::front_end::checked_program;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedScalarExpression, ClosedScalarContractValue,
};
use typed_trees::types::PrimitiveType;

fn ensures_clauses(machine_source: &str) -> Vec<Option<ClosedScalarContractValue>> {
    let source = format!("data Provider {{}}\n{machine_source}");
    let checked = checked_program(&source);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Provider::offset")
        .expect("the contracted machine");
    checked
        .facts
        .contract_plans
        .for_machine(machine.symbol)
        .expect("contract plan")
        .closed_scalar_values
        .ensures()
        .to_vec()
}

fn parameter(position: usize) -> Box<CheckedScalarExpression> {
    Box::new(CheckedScalarExpression::Parameter {
        position,
        primitive_type: PrimitiveType::I32,
    })
}

#[test]
fn result_equation_with_exact_literal_arithmetic_and_a_conjunct_is_one_predicate() {
    let clauses = ensures_clauses(
        "machine Provider::offset(input: i32) -> i32
         ensures result == input + 0 && input == input
         { transition { _ -> (input + 0) } }",
    );
    let [Some(ClosedScalarContractValue::Predicate(CheckedBooleanExpression::And { left, right }))] =
        clauses.as_slice()
    else {
        panic!("the ensures clause lowers to one conjunction predicate: {clauses:?}");
    };
    let CheckedBooleanExpression::IntegerComparison {
        kind: CheckedIntegerComparisonKind::Equal,
        left: result,
        right: sum,
    } = left.as_ref()
    else {
        panic!("the result equation is an integer equality: {left:?}");
    };
    assert_eq!(
        *result,
        parameter(1),
        "the reserved result follows the entry parameters"
    );
    let CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactAdd,
        primitive_type: PrimitiveType::I32,
        left: addend,
        right: literal,
    } = sum.as_ref()
    else {
        panic!("`input + 0` is exact i32 addition: {sum:?}");
    };
    assert_eq!(*addend, parameter(0));
    assert!(
        matches!(
            literal.as_ref(),
            CheckedScalarExpression::IntegerLiteral { .. }
        ),
        "the literal lands on the parameter's carrier: {literal:?}"
    );
    assert!(
        matches!(
            right.as_ref(),
            CheckedBooleanExpression::IntegerComparison {
                kind: CheckedIntegerComparisonKind::Equal,
                ..
            }
        ),
        "the tautology conjunct is an ordinary equality: {right:?}"
    );
}

#[test]
fn nested_exact_arithmetic_over_two_parameters_lowers() {
    let clauses = ensures_clauses(
        "machine Provider::offset(input: i32 [1..=10], scale: i32 [1..=10]) -> i32
         ensures result == input * scale - 1
         { transition { _ -> (input * scale - 1) } }",
    );
    let [
        Some(ClosedScalarContractValue::Predicate(CheckedBooleanExpression::IntegerComparison {
            right,
            ..
        })),
    ] = clauses.as_slice()
    else {
        panic!("{clauses:?}");
    };
    let CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactSubtract,
        left: product,
        ..
    } = right.as_ref()
    else {
        panic!("{right:?}");
    };
    assert!(matches!(
        product.as_ref(),
        CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactMultiply,
            ..
        }
    ));
}

#[test]
fn shapes_outside_the_closed_language_stay_unsupported() {
    // A wrapping carrier is not exact arithmetic; two literals have no carrier.
    for source in [
        "machine Provider::offset(input: i32 in Wrapping) -> i32 in Wrapping
         ensures result == input + 0
         { transition { _ -> (input + 0) } }",
        "machine Provider::offset(input: i32) -> i32
         ensures result == 1 + 0
         { transition { _ -> (1) } }",
    ] {
        let clauses = ensures_clauses(source);
        assert_eq!(clauses, vec![None], "{source}");
    }
}
