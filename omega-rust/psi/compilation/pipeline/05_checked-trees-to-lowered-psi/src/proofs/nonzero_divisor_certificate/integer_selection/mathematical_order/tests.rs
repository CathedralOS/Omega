use std::collections::BTreeSet;

use proof_admission::check_certificate;
use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

use crate::proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;

#[test]
fn mathematical_exit_order_retains_conservation_and_exact_guard() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let context = PropositionContext::from_value_types(
        (1..=6).map(|raw| (ValueId::new(raw).unwrap(), scalar)),
    )
    .unwrap();
    let value = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar);
    let mathematical = |raw| IntegerMathTerm::MathValue {
        source_type: integer,
        value: ValueId::new(raw).unwrap(),
    };
    let expression = |accumulator, limit, index| {
        IntegerMathTerm::Subtract(
            Box::new(IntegerMathTerm::Add(
                Box::new(mathematical(accumulator)),
                Box::new(mathematical(limit)),
            )),
            Box::new(mathematical(index)),
        )
    };
    let conservation = Proposition::IntegerMathEqual(expression(1, 2, 3), expression(4, 5, 6));
    let goal = Proposition::IntegerMathLessOrEqual(expression(1, 2, 3), mathematical(4));
    let requirements = [conservation.clone()];
    let guard = [Proposition::LessOrEqual(value(5), value(6))];
    let produce = |requirements: &[Proposition], guards: &[Proposition]| {
        produce_checked_canonical_integer_proof(
            &context,
            &goal,
            requirements,
            guards,
            &BTreeSet::new(),
            &mut crate::proofs::ProofSearchDeclines::default(),
        )
    };
    let proof = produce(&requirements, &guard).expect("conserved expression meets the exit bound");
    check_certificate(&context, &goal, &requirements, &guard, &proof).unwrap();
    let aliased = [guard[0].clone(), Proposition::Equal(value(2), value(5))];
    let normalized_goal = Proposition::IntegerMathLessOrEqual(expression(1, 5, 3), mathematical(4));
    let normalized = produce_checked_canonical_integer_proof(
        &context,
        &normalized_goal,
        &requirements,
        &aliased,
        &BTreeSet::new(),
        &mut crate::proofs::ProofSearchDeclines::default(),
    )
    .expect("exit order retains original conservation through an entry/header alias");
    check_certificate(
        &context,
        &normalized_goal,
        &requirements,
        &aliased,
        &normalized,
    )
    .unwrap();
    assert!(
        check_certificate(
            &context,
            &normalized_goal,
            &requirements,
            &guard,
            &normalized
        )
        .is_err()
    );
    let wrong_alias = [guard[0].clone(), Proposition::Equal(value(2), value(6))];
    assert!(
        check_certificate(
            &context,
            &normalized_goal,
            &requirements,
            &wrong_alias,
            &normalized
        )
        .is_err()
    );
    assert!(produce(&[], &guard).is_none());
    assert!(produce(&requirements, &[]).is_none());
    let reversed = [Proposition::LessOrEqual(value(6), value(5))];
    assert!(produce(&requirements, &reversed).is_none());
    assert!(check_certificate(&context, &goal, &requirements, &reversed, &proof).is_err());
    let conditional = [Proposition::Implication {
        premise: Box::new(Proposition::Falsehood),
        conclusion: Box::new(guard[0].clone()),
    }];
    assert!(produce(&requirements, &conditional).is_none());
    let wrong_conservation = [Proposition::IntegerMathEqual(
        expression(1, 2, 3),
        expression(4, 5, 3),
    )];
    assert!(produce(&wrong_conservation, &guard).is_none());
    assert!(
        check_certificate(
            &PropositionContext::default(),
            &goal,
            &requirements,
            &guard,
            &proof
        )
        .is_err()
    );
}

#[test]
fn correlated_exact_endpoint_order_replays_definitions_and_policy() {
    use semantic_vocabulary::IntegerValue;

    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let context = PropositionContext::from_value_types(
        (1..=5).map(|raw| (ValueId::new(raw).unwrap(), scalar)),
    )
    .unwrap();
    let value = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar);
    let literal = |number| ScalarTerm::integer(integer, IntegerValue::Unsigned(number)).unwrap();
    let exact = |operand, amount| ScalarTerm::exact_integer_add(integer, operand, amount).unwrap();
    let assumptions = [Proposition::LessOrEqual(value(1), value(2))];
    let produce = |goal: &Proposition, assumptions: &[Proposition], definitions: &[Proposition]| {
        produce_checked_canonical_integer_proof(
            &context,
            goal,
            assumptions,
            definitions,
            &BTreeSet::new(),
            &mut crate::proofs::ProofSearchDeclines::default(),
        )
    };
    let direct = Proposition::LessOrEqual(exact(value(1), literal(1)), exact(value(2), literal(1)));
    let direct_proof =
        produce(&direct, &assumptions, &[]).expect("same exact translation preserves the order");
    check_certificate(&context, &direct, &assumptions, &[], &direct_proof).unwrap();

    // The actual backedge names two current results and a separately landed
    // literal. Only their original SSA equations connect them to the header.
    let definitions = [
        Proposition::Equal(value(5), literal(1)),
        Proposition::Equal(value(3), exact(value(1), value(5))),
        Proposition::Equal(value(4), exact(value(2), value(5))),
    ];
    let goal = Proposition::LessOrEqual(value(3), value(4));
    let proof = produce(&goal, &assumptions, &definitions)
        .expect("current result definitions retain correlated order");
    check_certificate(&context, &goal, &assumptions, &definitions, &proof).unwrap();
    let reversed = [Proposition::LessOrEqual(value(2), value(1))];
    assert!(produce(&goal, &reversed, &definitions).is_none());
    assert!(check_certificate(&context, &goal, &reversed, &definitions, &proof).is_err());
    assert!(produce(&goal, &assumptions, &definitions[..2]).is_none());
    assert!(check_certificate(&context, &goal, &assumptions, &definitions[..2], &proof).is_err());
    let conditional = [Proposition::Implication {
        premise: Box::new(Proposition::Falsehood),
        conclusion: Box::new(assumptions[0].clone()),
    }];
    assert!(produce(&goal, &conditional, &definitions).is_none());

    // An unequal increment does not preserve this direction, and modular
    // overflow can reverse even an equal-increment order (254 <= 255).
    let wrong_delta =
        Proposition::LessOrEqual(exact(value(1), literal(2)), exact(value(2), literal(1)));
    assert!(produce(&wrong_delta, &assumptions, &[]).is_none());
    let wrapping = Proposition::LessOrEqual(
        ScalarTerm::wrapping_integer_add(integer, value(1), literal(1)).unwrap(),
        ScalarTerm::wrapping_integer_add(integer, value(2), literal(1)).unwrap(),
    );
    assert!(produce(&wrapping, &assumptions, &[]).is_none());
    // Saturation is not an exact-ring operation. This unequal translation
    // also has a concrete counterexample at equal zero endpoints.
    let saturating = Proposition::LessOrEqual(
        ScalarTerm::saturating_integer_add(integer, value(1), literal(2)).unwrap(),
        ScalarTerm::saturating_integer_add(integer, value(2), literal(1)).unwrap(),
    );
    assert!(produce(&saturating, &assumptions, &[]).is_none());
    let wrong_scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).unwrap());
    let wrong_context = PropositionContext::from_value_types(
        (1..=5).map(|raw| (ValueId::new(raw).unwrap(), wrong_scalar)),
    )
    .unwrap();
    assert!(check_certificate(&wrong_context, &goal, &assumptions, &definitions, &proof).is_err());
}
