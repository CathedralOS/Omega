use super::{BoundedIntegerType, PropositionContext, ScalarType, declared_carrier_bounds};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ValueId,
};

fn value(index: u64, scalar_type: ScalarType) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(index).unwrap(), scalar_type)
}

/// The proposition context a certificate check runs under: the `Value` leaf
/// keeps its declared type.
fn context(value: &ScalarTerm) -> PropositionContext {
    let ScalarTerm::Value { id, scalar_type } = value else {
        panic!("the bound subject is a value term")
    };
    PropositionContext::from_value_types([(*id, *scalar_type)]).unwrap()
}

fn declared(
    sign: IntegerSign,
    bits: u16,
    minimum: i128,
    maximum: i128,
) -> (BoundedIntegerType, ScalarTerm) {
    let integer_type = IntegerType::new(sign, bits).unwrap();
    let literal = |literal: i128| match sign {
        IntegerSign::Signed => IntegerValue::Signed(literal),
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::try_from(literal).unwrap()),
    };
    (
        BoundedIntegerType::new(integer_type, literal(minimum), literal(maximum)).unwrap(),
        value(1, ScalarType::Integer(integer_type)),
    )
}

#[test]
fn declared_bounds_carry_accepted_certificates() {
    for (sign, bits, minimum, maximum) in [
        (IntegerSign::Unsigned, 8, 0, 200),
        (IntegerSign::Signed, 32, -50, 50),
        (IntegerSign::Unsigned, 64, 0, i128::from(u64::MAX)),
    ] {
        let (bounds, value) = declared(sign, bits, minimum, maximum);
        let integer_type = bounds.integer_type();
        let expected = [
            Proposition::LessOrEqual(
                ScalarTerm::integer(integer_type, bounds.minimum()).unwrap(),
                value.clone(),
            ),
            Proposition::LessOrEqual(
                value.clone(),
                ScalarTerm::integer(integer_type, bounds.maximum()).unwrap(),
            ),
        ];
        let facts = declared_carrier_bounds(&context(&value), bounds, value);
        assert_eq!(
            facts.each_ref().map(|fact| &fact.proposition),
            expected.each_ref()
        );
        assert!(
            facts.iter().all(|fact| fact.certified),
            "every emitted declared bound re-decides under the checker"
        );
    }
}

#[test]
fn a_bound_outside_the_declared_interval_is_not_certified() {
    let (bounds, value) = declared(IntegerSign::Unsigned, 8, 5, 9);
    let integer_type = bounds.integer_type();
    let mut members = vec![
        Proposition::LessOrEqual(
            ScalarTerm::integer(integer_type, bounds.minimum()).unwrap(),
            value.clone(),
        ),
        Proposition::LessOrEqual(
            value.clone(),
            ScalarTerm::integer(integer_type, bounds.maximum()).unwrap(),
        ),
    ];
    members.sort();
    let declared_invariant = Proposition::Conjunction(members);
    // The declared interval [5, 9] bounds the value by 9, not by 99: the
    // foreign bound is no conjunct of the invariant, so no elimination
    // certificate exists for it and the emission would remain a licensed
    // premise under the calling row.
    let foreign = Proposition::LessOrEqual(
        value.clone(),
        ScalarTerm::integer(integer_type, IntegerValue::Unsigned(99)).unwrap(),
    );
    assert!(!super::declared_bound_certified(
        &context(&value),
        &declared_invariant,
        &foreign
    ));
}
