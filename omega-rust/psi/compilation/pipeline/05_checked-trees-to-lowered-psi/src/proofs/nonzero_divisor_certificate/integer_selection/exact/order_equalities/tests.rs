use proof_admission::accept_certificate;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

use super::super::{prove, session};

fn carrier(sign: IntegerSign) -> IntegerType {
    IntegerType::new(sign, 64).unwrap()
}

fn value(carrier: IntegerType, identity: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(carrier),
    )
}

fn context(carrier: IntegerType) -> PropositionContext {
    PropositionContext::from_value_types((1..=3).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(carrier),
        )
    }))
    .unwrap()
}

#[test]
fn order_equality_connects_arbitrary_endpoints_with_both_citations() {
    let carrier = carrier(IntegerSign::Signed);
    let context = context(carrier);
    let left = value(carrier, 1);
    let right = value(carrier, 2);
    let facts = vec![
        Proposition::LessOrEqual(left.clone(), right.clone()),
        Proposition::LessOrEqual(right.clone(), left.clone()),
    ];
    let goal = Proposition::Equal(left, right);
    let proof = prove(&context, &goal, &[], &facts).expect("opposite bounds connect scalar values");
    accept_certificate(&context, &goal, &[], &facts, &proof).unwrap();
    for missing in 0..facts.len() {
        let mut incomplete = facts.clone();
        incomplete[missing] = Proposition::Truth;
        assert!(prove(&context, &goal, &[], &incomplete).is_none());
        assert!(accept_certificate(&context, &goal, &[], &incomplete, &proof).is_err());
    }
}

#[test]
fn order_equality_transports_a_carrier_endpoint_through_wrapping_terms() {
    let carrier = carrier(IntegerSign::Unsigned);
    let context = context(carrier);
    let subject = value(carrier, 1);
    let zero = ScalarTerm::integer(carrier, IntegerValue::Unsigned(0)).unwrap();
    let one = ScalarTerm::integer(carrier, IntegerValue::Unsigned(1)).unwrap();
    let facts = vec![Proposition::LessOrEqual(subject.clone(), zero.clone())];
    let polynomial = ScalarTerm::wrapping_integer_multiply(
        carrier,
        subject.clone(),
        ScalarTerm::wrapping_integer_add(carrier, subject.clone(), one).unwrap(),
    )
    .unwrap();
    let goal = Proposition::Equal(polynomial, zero.clone());
    let proof = prove(&context, &goal, &[], &facts)
        .expect("carrier and exit prove zero before ring normalization");
    accept_certificate(&context, &goal, &[], &facts, &proof).unwrap();
    let literal = session(&context, &[], &facts)
        .literal_equality(&subject)
        .unwrap();
    accept_certificate(
        &context,
        &Proposition::Equal(subject, zero),
        &[],
        &facts,
        &literal,
    )
    .unwrap();
    assert!(prove(&context, &goal, &[], &[]).is_none());
    assert!(accept_certificate(&context, &goal, &[], &[], &proof).is_err());
}

#[test]
fn order_equality_never_uses_a_conditional_or_a_missing_signed_lower_bound() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let carrier = carrier(sign);
        let context = context(carrier);
        let subject = value(carrier, 1);
        let zero = ScalarTerm::integer(
            carrier,
            match sign {
                IntegerSign::Signed => IntegerValue::Signed(0),
                IntegerSign::Unsigned => IntegerValue::Unsigned(0),
            },
        )
        .unwrap();
        let bound = Proposition::LessOrEqual(subject.clone(), zero.clone());
        let goal = Proposition::Equal(subject, zero);
        if sign == IntegerSign::Signed {
            assert!(prove(&context, &goal, &[], &[bound.clone()]).is_none());
        }
        let conditional = Proposition::Implication {
            premise: Box::new(Proposition::Falsehood),
            conclusion: Box::new(bound),
        };
        assert!(prove(&context, &goal, &[], &[conditional]).is_none());
    }
}
