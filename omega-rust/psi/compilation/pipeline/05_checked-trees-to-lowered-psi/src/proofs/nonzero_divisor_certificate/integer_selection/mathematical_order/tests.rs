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

#[test]
fn correlated_sum_bound_composes_translated_order_with_retained_upper_bound() {
    use semantic_vocabulary::{IntegerMathLiteral, IntegerValue};

    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let context = PropositionContext::from_value_types(
        (1..=4).map(|raw| (ValueId::new(raw).unwrap(), scalar)),
    )
    .unwrap();
    let value = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar);
    let literal = |number| ScalarTerm::integer(integer, IntegerValue::Unsigned(number)).unwrap();
    let math_value = |raw| IntegerMathTerm::MathValue {
        source_type: integer,
        value: ValueId::new(raw).unwrap(),
    };
    let math_literal =
        |number| IntegerMathTerm::IntegerLiteral(IntegerMathLiteral::new(false, number).unwrap());
    let goal = |maximum| {
        Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Add(Box::new(math_value(2)), Box::new(math_value(4))),
            math_literal(maximum),
        )
    };
    // remaining, accumulator, total, and the executed literal one. The
    // overflow question concerns accumulator + 1; its bound is correlated
    // with remaining through the CURRENT sum, not the invocation inputs.
    let facts = vec![
        Proposition::LessOrEqual(literal(1), value(1)),
        Proposition::Equal(
            value(3),
            ScalarTerm::exact_integer_add(integer, value(1), value(2)).unwrap(),
        ),
        Proposition::LessOrEqual(value(3), literal(100)),
        Proposition::Equal(value(4), literal(1)),
    ];
    let produce = |goal: &Proposition, facts: &[Proposition]| {
        produce_checked_canonical_integer_proof(
            &context,
            goal,
            &[],
            facts,
            &BTreeSet::new(),
            &mut crate::proofs::ProofSearchDeclines::default(),
        )
    };
    for maximum in [100, 255] {
        let goal = goal(maximum);
        let proof =
            produce(&goal, &facts).expect("translate the lower bound, then compose the sum bound");
        proof_admission::accept_certificate(&context, &goal, &[], &facts, &proof)
            .expect("the existing checked rules and mathematical core replay the full chain");
    }
    let strong_goal = goal(100);
    let proof = produce(&strong_goal, &facts).unwrap();
    for missing in 0..facts.len() {
        let mut incomplete = facts.clone();
        incomplete.remove(missing);
        assert!(
            produce(&strong_goal, &incomplete).is_none(),
            "missing premise {missing}"
        );
        assert!(
            proof_admission::accept_certificate(&context, &strong_goal, &[], &incomplete, &proof)
                .is_err()
        );
    }
    for (position, replacement) in [
        (0, Proposition::LessOrEqual(value(1), literal(1))),
        (
            1,
            Proposition::Equal(
                value(3),
                ScalarTerm::exact_integer_add(integer, value(1), value(1)).unwrap(),
            ),
        ),
        (2, Proposition::LessOrEqual(literal(100), value(3))),
        (3, Proposition::Equal(value(4), literal(2))),
    ] {
        let mut changed = facts.clone();
        changed[position] = replacement;
        assert!(
            produce(&strong_goal, &changed).is_none(),
            "substituted premise {position}"
        );
        assert!(
            proof_admission::accept_certificate(&context, &strong_goal, &[], &changed, &proof)
                .is_err()
        );
    }
    // Neither modular nor saturating addition gives an unbounded sum.
    // At remaining=1, accumulator=255, either policy can satisfy total<=255
    // while the requested mathematical accumulator+1<=255 is false.
    for sum in [
        ScalarTerm::wrapping_integer_add(integer, value(1), value(2)).unwrap(),
        ScalarTerm::saturating_integer_add(integer, value(1), value(2)).unwrap(),
    ] {
        let mut changed = facts.clone();
        changed[1] = Proposition::Equal(value(3), sum);
        changed[2] = Proposition::LessOrEqual(value(3), literal(255));
        assert!(
            produce(&goal(255), &changed).is_none(),
            "policy arithmetic cannot acquire exact sum order"
        );
    }
}

#[test]
fn correlated_exit_equality_replays_current_updates_and_both_order_directions() {
    use semantic_vocabulary::IntegerValue;

    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let context = PropositionContext::from_value_types(
        (1..=7).map(|raw| (ValueId::new(raw).unwrap(), scalar)),
    )
    .unwrap();
    let value = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar);
    let literal = |number| ScalarTerm::integer(integer, IntegerValue::Unsigned(number)).unwrap();
    let add = |left, right| ScalarTerm::exact_integer_add(integer, left, right).unwrap();
    let subtract = |left, right| ScalarTerm::exact_integer_subtract(integer, left, right).unwrap();
    // Header remaining and accumulator are 1 and 2; 3 is their conserved
    // total. The exit compares the updated remaining (4), then returns the
    // updated accumulator (5) through a distinct result identity (6).
    let facts = vec![
        Proposition::LessOrEqual(literal(1), value(1)),
        Proposition::Equal(value(3), add(value(1), value(2))),
        Proposition::Equal(value(7), literal(1)),
        Proposition::Equal(value(4), subtract(value(1), value(7))),
        Proposition::LessOrEqual(value(4), literal(0)),
        Proposition::Equal(value(5), add(value(2), value(7))),
        Proposition::Equal(value(6), value(5)),
    ];
    let goal = Proposition::Equal(value(3), value(6));
    let produce = |goal: &Proposition, facts: &[Proposition]| {
        produce_checked_canonical_integer_proof(
            &context,
            goal,
            &[],
            facts,
            &BTreeSet::new(),
            &mut crate::proofs::ProofSearchDeclines::default(),
        )
    };
    for goal in [goal.clone(), Proposition::Equal(value(6), value(3))] {
        let proof = produce(&goal, &facts)
            .expect("opposite translated orders prove the conserved exit equality");
        proof_admission::accept_certificate(&context, &goal, &[], &facts, &proof)
            .expect("the independent kernel and mathematical core replay both directions");
    }
    let proof = produce(&goal, &facts).unwrap();
    // Each omitted row leaves a genuine unconstrained identity or admits a
    // different exit value. Keep the lower bound and shared increment: unsigned
    // exact subtraction supplies nonnegativity, and equal symbolic increments
    // can preserve conservation without knowing their literal value.
    for missing in [1, 3, 4, 5, 6] {
        let mut incomplete = facts.clone();
        incomplete.remove(missing);
        assert!(
            produce(&goal, &incomplete).is_none(),
            "missing exit fact {missing}"
        );
        assert!(
            proof_admission::accept_certificate(&context, &goal, &[], &incomplete, &proof).is_err()
        );
    }
    for (position, replacement) in [
        (
            3,
            Proposition::Equal(value(4), subtract(value(1), literal(2))),
        ),
        (4, Proposition::LessOrEqual(literal(0), value(4))),
        (5, Proposition::Equal(value(5), add(value(2), literal(2)))),
        (6, Proposition::Equal(value(6), value(2))),
    ] {
        let mut changed = facts.clone();
        changed[position] = replacement;
        assert!(
            produce(&goal, &changed).is_none(),
            "substituted exit fact {position}"
        );
        assert!(
            proof_admission::accept_certificate(&context, &goal, &[], &changed, &proof).is_err()
        );
    }
    // Changing the result policy must not reuse the exact-ring certificate.
    // Increment two also makes both policy variants false at remaining=1,
    // accumulator=0; this does not demand rejection of a valid no-wrap identity.
    for result in [
        ScalarTerm::wrapping_integer_add(integer, value(2), literal(2)).unwrap(),
        ScalarTerm::saturating_integer_add(integer, value(2), literal(2)).unwrap(),
    ] {
        let mut changed = facts.clone();
        changed[5] = Proposition::Equal(value(5), result);
        assert!(produce(&goal, &changed).is_none());
        assert!(
            proof_admission::accept_certificate(&context, &goal, &[], &changed, &proof).is_err()
        );
    }
}
