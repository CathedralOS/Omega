use std::collections::BTreeSet;

use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, PropositionContext, ScalarType, ValueId,
};

use super::super::{Elaboration, MathTermKey};
use super::{Constructor, IntegerValue, Law, Proposition, ScalarTerm, Term};
use crate::{
    Budget, DEFAULT_CONVERSION_STEPS, PrimitiveJudgment, ProofNode, ProofRule,
    certificate_assumption_closure, verify_bounded_certificate, verify_mathematical_certificate,
};

fn fixture(
    sign: IntegerSign,
    lower: IntegerValue,
    upper: IntegerValue,
    lower_bound: bool,
) -> (PropositionContext, Proposition, ProofNode) {
    let integer = IntegerType::new(sign, 128).unwrap();
    let scalar = ScalarType::Integer(integer);
    let identity = ValueId::new(1).unwrap();
    let value = ScalarTerm::value(identity, scalar);
    let lower = ScalarTerm::integer(integer, lower).unwrap();
    let upper = ScalarTerm::integer(integer, upper).unwrap();
    let (premise, conclusion) = if lower_bound {
        (
            Proposition::LessOrEqual(upper, value.clone()),
            Proposition::LessThan(lower, value),
        )
    } else {
        (
            Proposition::LessOrEqual(value.clone(), lower),
            Proposition::LessThan(value, upper),
        )
    };
    let proof = ProofNode {
        conclusion,
        rule: ProofRule::IntegerOrderDiscreteness {
            relation: Box::new(ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
        },
    };
    (
        PropositionContext::from_value_types([(identity, scalar)]).unwrap(),
        premise,
        proof,
    )
}

#[test]
fn binary_carry_and_signed_zero_use_exact_shared_law_closures() {
    for (lower, upper, expected_laws) in [
        (0, 1, vec![Law::DoubleZero, Law::EvenBeforeOdd]),
        (2, 3, vec![Law::EvenBeforeOdd]),
        (
            7,
            8,
            vec![Law::DoubleZero, Law::EvenBeforeOdd, Law::OddBeforeNextEven],
        ),
        (
            -2,
            -1,
            vec![
                Law::DoubleZero,
                Law::EvenBeforeOdd,
                Law::OddBeforeNextEven,
                Law::NegateOrder,
            ],
        ),
        (
            -1,
            0,
            vec![
                Law::DoubleZero,
                Law::NegateZero,
                Law::EvenBeforeOdd,
                Law::NegateOrder,
            ],
        ),
    ] {
        for lower_bound in [false, true] {
            let (context, premise, proof) = fixture(
                IntegerSign::Signed,
                IntegerValue::Signed(lower),
                IntegerValue::Signed(upper),
                lower_bound,
            );
            let premises = [premise];
            let parameters = BTreeSet::new();
            let mut elaboration =
                Elaboration::new(&context, &proof.conclusion, &premises, &[], &parameters).unwrap();
            let evidence = elaboration.node(&proof).unwrap();
            let denotation = &elaboration.denotation;
            assert!(
                denotation.rule_axioms.is_empty(),
                "discreteness must not assume its instance"
            );
            assert!(
                denotation.decisions.is_empty(),
                "literal adjacency must not become a primitive decision assumption"
            );
            assert_eq!(
                denotation
                    .binary_numerals
                    .laws
                    .keys()
                    .copied()
                    .collect::<Vec<_>>(),
                expected_laws
            );

            let mut expected_closure = BTreeSet::from([
                denotation.integer.unwrap(),
                denotation.integer_less_than.unwrap(),
                denotation.integer_less_or_equal.unwrap(),
                denotation.binary_numerals.zero.unwrap(),
                denotation.binary_numerals.constructors[&Constructor::Double],
                denotation.binary_numerals.constructors[&Constructor::Odd],
            ]);
            if lower < 0 {
                expected_closure
                    .insert(denotation.binary_numerals.constructors[&Constructor::Negate]);
            }
            expected_closure.extend(denotation.binary_numerals.laws.values());
            expected_closure.extend(denotation.integer_laws.values());
            expected_closure.extend(denotation.math_terms.iter().filter_map(
                |(term, &position)| matches!(term, MathTermKey::Open(_)).then_some(position),
            ));
            let mut denoted = elaboration.finish(evidence);
            verify_mathematical_certificate(
                &mut denoted.arena,
                &denoted.certificate,
                &mut Budget::default(),
            )
            .unwrap();
            assert_eq!(
                certificate_assumption_closure(&denoted.arena, &denoted.certificate),
                expected_closure
            );
            let mut malformed = denoted.certificate.clone();
            malformed.term = denoted.arena.insert(Term::Variable(0));
            assert!(
                verify_mathematical_certificate(
                    &mut denoted.arena,
                    &malformed,
                    &mut Budget::default()
                )
                .is_err(),
                "the kernel must reject the inclusive premise as strict evidence"
            );
        }
    }
}

#[test]
fn maximum_width_discreteness_checks_with_the_existing_budget() {
    for (sign, lower, upper) in [
        (
            IntegerSign::Unsigned,
            IntegerValue::Unsigned(u128::MAX - 1),
            IntegerValue::Unsigned(u128::MAX),
        ),
        (
            IntegerSign::Unsigned,
            IntegerValue::Unsigned((1_u128 << 127) - 1),
            IntegerValue::Unsigned(1_u128 << 127),
        ),
        (
            IntegerSign::Signed,
            IntegerValue::Signed(i128::MIN),
            IntegerValue::Signed(i128::MIN + 1),
        ),
        (
            IntegerSign::Signed,
            IntegerValue::Signed(i128::MAX - 1),
            IntegerValue::Signed(i128::MAX),
        ),
    ] {
        for lower_bound in [false, true] {
            let (context, premise, proof) = fixture(sign, lower, upper, lower_bound);
            let mut budget = Budget::default();
            let denoted = verify_bounded_certificate(
                &context,
                &proof.conclusion,
                &[premise],
                &[],
                &proof,
                &mut budget,
            )
            .unwrap();
            eprintln!(
                "{lower:?} lower_bound={lower_bound}: steps={} receipt={:?}",
                DEFAULT_CONVERSION_STEPS - budget.remaining(),
                denoted.receipt()
            );
            assert!(
                denoted.certificate.signature.len() < 400,
                "shared prefixes stay proportional to the 128-bit endpoints"
            );
        }
    }
}

#[test]
fn large_closed_values_keep_exact_opaque_identity_without_a_new_capacity_limit() {
    let literal = |value| IntegerMathTerm::literal(IntegerValue::Unsigned(value));
    let huge = IntegerMathTerm::ShiftLeft {
        value: Box::new(literal(1)),
        count: Box::new(literal(65_535)),
    };
    // Both shift spellings stay inside the evaluator's bit estimate. Adding
    // zero at the bit ceiling conservatively reserves another result bit.
    let alternate = IntegerMathTerm::ShiftLeft {
        value: Box::new(literal(2)),
        count: Box::new(literal(65_534)),
    };
    let mut terms = [huge, alternate];
    terms.sort();
    let goal = Proposition::IntegerMathEqual(terms[0].clone(), terms[1].clone());
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    };
    let denoted = verify_bounded_certificate(
        &PropositionContext::default(),
        &goal,
        &[],
        &[],
        &proof,
        &mut Budget::default(),
    )
    .unwrap();
    assert_eq!(
        denoted.certificate.signature.len(),
        2,
        "carrier and one exact-value constant, as before binary numeral denotation"
    );
    assert_eq!(
        certificate_assumption_closure(&denoted.arena, &denoted.certificate),
        BTreeSet::from([0, 1])
    );
    assert!(matches!(
        denoted.arena.get(denoted.certificate.term),
        Term::Refl { .. }
    ));
}

#[test]
fn maximum_fixed_literals_and_closed_expressions_share_reflexive_definitions() {
    for value in [
        IntegerValue::Unsigned(u128::MAX),
        IntegerValue::Signed(i128::MIN),
        IntegerValue::Unsigned(0),
    ] {
        let literal = IntegerMathTerm::literal(value);
        let expression = IntegerMathTerm::Add(
            Box::new(literal.clone()),
            Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(0))),
        );
        let mut terms = [literal, expression];
        terms.sort();
        let goal = Proposition::IntegerMathEqual(terms[0].clone(), terms[1].clone());
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        };
        let mut budget = Budget::default();
        let denoted = verify_bounded_certificate(
            &PropositionContext::default(),
            &goal,
            &[],
            &[],
            &proof,
            &mut budget,
        )
        .unwrap();
        assert!(matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::Refl { .. }
        ));
        assert!(denoted.certificate.signature.len() <= 134);
        eprintln!(
            "equality {value:?}: steps={} receipt={:?}",
            DEFAULT_CONVERSION_STEPS - budget.remaining(),
            denoted.receipt()
        );
    }
}
