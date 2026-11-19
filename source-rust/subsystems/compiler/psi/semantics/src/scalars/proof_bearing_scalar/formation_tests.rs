use super::*;
use semantic_vocabulary::{IntegerMathTerm, IntegerSign, IntegerValue};

#[test]
fn computed_operands_keep_their_policy_and_validate_before_simplification() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let literal =
        |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
    let value = ScalarTerm::value(ValueId::new(17).unwrap(), ScalarType::Integer(integer));
    let factor = ScalarTerm::value(ValueId::new(18).unwrap(), ScalarType::Integer(integer));
    let product =
        ScalarTerm::exact_integer_multiply(integer, value.clone(), factor).unwrap();
    let nested =
        ScalarTerm::exact_integer_add(integer, product.clone(), literal(1)).unwrap();
    let goal = |term: &ScalarTerm| {
        CanonicalScalarGoal::for_term(term)
            .unwrap()
            .unwrap()
            .kernel_proposition()
    };
    let mut retained = false;
    goal(&nested)
        .unwrap()
        .visit_scalar_terms(|term| retained |= term == &product);
    assert!(
        retained,
        "the inner computation remains in the formation question"
    );

    let wrapped = ScalarTerm::wrapping_integer_add(integer, value, literal(1)).unwrap();
    let nested =
        ScalarTerm::exact_integer_subtract(integer, wrapped.clone(), literal(1)).unwrap();
    let mut retained = false;
    goal(&nested)
        .unwrap()
        .visit_scalar_terms(|term| retained |= term == &wrapped);
    assert!(
        retained,
        "embedding must preserve the operand's wrapping policy"
    );

    let malformed = ScalarTerm::ExactIntegerMultiply {
        scalar_type: integer,
        left: Box::new(ScalarTerm::ExactIntegerAdd {
            scalar_type: integer,
            left: Box::new(ScalarTerm::Boolean(true)),
            right: Box::new(literal(1)),
        }),
        right: Box::new(literal(0)),
    };
    assert!(
        goal(&malformed).is_err(),
        "zero cannot hide malformed nested types"
    );
}

#[test]
fn terms_keep_their_own_formation_conditions() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let literal =
        |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
    let offset = ValueId::new(17).unwrap();
    let difference = ScalarTerm::exact_integer_subtract(
        integer,
        literal(255),
        ScalarTerm::value(offset, ScalarType::Integer(integer)),
    )
    .unwrap();
    assert_eq!(
        CanonicalScalarGoal::for_term(&difference)
            .unwrap()
            .unwrap()
            .kernel_proposition()
            .unwrap(),
        Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
            IntegerMathTerm::Subtract(
                Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(255))),
                Box::new(IntegerMathTerm::MathValue {
                    source_type: integer,
                    value: offset,
                }),
            ),
        )
    );
    let underflow =
        ScalarTerm::exact_integer_subtract(integer, literal(0), literal(1)).unwrap();
    assert_eq!(
        CanonicalScalarGoal::for_term(&underflow)
            .unwrap()
            .unwrap()
            .kernel_proposition()
            .unwrap(),
        Proposition::Falsehood,
    );
    let division =
        ScalarTerm::wrapping_integer_divide(integer, literal(3), literal(0)).unwrap();
    assert_eq!(
        CanonicalScalarGoal::for_term(&division)
            .unwrap()
            .unwrap()
            .kernel_proposition()
            .unwrap(),
        Proposition::LessOrEqual(literal(1), literal(0)),
        "wrapping does not define division by zero",
    );
    let addition =
        ScalarTerm::wrapping_integer_add(integer, literal(255), literal(1)).unwrap();
    assert_eq!(CanonicalScalarGoal::for_term(&addition).unwrap(), None);
    let malformed = ScalarTerm::ExactIntegerSubtract {
        scalar_type: integer,
        left: Box::new(ScalarTerm::Boolean(true)),
        right: Box::new(literal(1)),
    };
    assert!(CanonicalScalarGoal::for_term(&malformed).is_err());
}
