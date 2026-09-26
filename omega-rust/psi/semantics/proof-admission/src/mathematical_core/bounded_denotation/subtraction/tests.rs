use std::collections::BTreeSet;

use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

use super::super::Elaboration;
use crate::{
    Budget, PrimitiveJudgment, ProofNode, ProofRule, Term, verify_bounded_certificate,
    verify_mathematical_certificate,
};

fn fixture(
    integer: IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
) -> (PropositionContext, Vec<Proposition>, ProofNode) {
    let scalar = ScalarType::Integer(integer);
    let result = ScalarTerm::value(ValueId::new(1).unwrap(), scalar);
    let difference = Proposition::Equal(
        result.clone(),
        ScalarTerm::exact_integer_subtract(integer, left.clone(), right.clone()).unwrap(),
    );
    let zero = match integer.sign() {
        IntegerSign::Signed => IntegerValue::Signed(0),
        IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    };
    let positive = Proposition::LessThan(ScalarTerm::integer(integer, zero).unwrap(), right);
    let assumptions = vec![difference.clone(), positive.clone()];
    let proof = ProofNode {
        conclusion: Proposition::LessThan(result, left),
        rule: ProofRule::IntegerSubtractOrder {
            difference: Box::new(ProofNode {
                conclusion: difference,
                rule: ProofRule::Assumption { index: 0 },
            }),
            positive: Box::new(ProofNode {
                conclusion: positive,
                rule: ProofRule::Assumption { index: 1 },
            }),
        },
    };
    let context = PropositionContext::from_value_types(
        (1..=3).map(|value| (ValueId::new(value).unwrap(), scalar)),
    )
    .unwrap();
    (context, assumptions, proof)
}

fn checked(integer: IntegerType, left: ScalarTerm, right: ScalarTerm) {
    let (context, assumptions, proof) = fixture(integer, left, right);
    let parameters = BTreeSet::new();
    let mut elaboration =
        Elaboration::new(&context, &proof.conclusion, &assumptions, &[], &parameters).unwrap();
    elaboration.node(&proof).unwrap();
    assert!(elaboration.denotation.rule_axioms.is_empty());
    assert!(elaboration.denotation.decisions.is_empty());
    let mut budget = Budget::default();
    let mut denoted = verify_bounded_certificate(
        &context,
        &proof.conclusion,
        &assumptions,
        &[],
        &proof,
        &mut budget,
    )
    .unwrap();
    eprintln!(
        "conditional subtraction: {:?}, remaining={}",
        denoted.receipt(),
        budget.remaining()
    );
    assert!(denoted.certificate.signature.len() < 800);
    let evidence = denoted.certificate.term;
    denoted.certificate.term = denoted.arena.insert(Term::TwoZero);
    assert!(
        verify_mathematical_certificate(
            &mut denoted.arena,
            &denoted.certificate,
            &mut Budget::default()
        )
        .is_err()
    );
    denoted.certificate.term = evidence;
    verify_mathematical_certificate(
        &mut denoted.arena,
        &denoted.certificate,
        &mut Budget::default(),
    )
    .unwrap();
    assert!(
        verify_bounded_certificate(
            &context,
            &proof.conclusion,
            &assumptions[..1],
            &[],
            &proof,
            &mut Budget::default()
        )
        .is_err()
    );
}

#[test]
fn conditional_subtraction_preserves_open_closed_and_false_positive_premises() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).unwrap();
    let value = |identity| {
        ScalarTerm::value(
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer),
        )
    };
    checked(integer, value(2), value(3));
    for (left, right) in [
        (5, 1),
        (5, 7),
        (7, 4),
        (8, 6),
        (8, 5),
        (5, 0),
        (5, -2),
        (-128, 1),
        (127, -1),
        (-5, 7),
        (0, 127),
        (-128, 0),
    ] {
        checked(integer, literal(left), literal(right));
    }
    checked(
        integer,
        ScalarTerm::exact_integer_add(integer, value(2), literal(1)).unwrap(),
        value(3),
    );
}

#[test]
fn maximal_width_and_nested_overflow_keep_the_original_conditional_domain() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
    checked(integer, literal(u128::MAX), literal(u128::MAX));
    checked(integer, literal(0), literal(1));
    let negative_one = ScalarTerm::exact_integer_subtract(integer, literal(0), literal(1)).unwrap();
    // The mathematical result is -2^128, outside the fixed numeral magnitude
    // range; exact scalar evaluation is undefined and stays compositional.
    checked(integer, negative_one, literal(u128::MAX));
    let signed = IntegerType::new(IntegerSign::Signed, 128).unwrap();
    let literal = |value| ScalarTerm::integer(signed, IntegerValue::Signed(value)).unwrap();
    checked(signed, literal(i128::MAX), literal(i128::MAX));
    checked(signed, literal(i128::MIN), literal(0));
    checked(signed, literal(0), literal(i128::MIN));
}

#[test]
fn closed_subtraction_still_denotes_the_canonical_evaluated_value() {
    let integer = IntegerType::new(IntegerSign::Signed, 128).unwrap();
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).unwrap();
    let subtraction = ScalarTerm::exact_integer_subtract(integer, literal(9), literal(4)).unwrap();
    let goal = Proposition::Equal(subtraction, literal(5));
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    };
    verify_bounded_certificate(
        &PropositionContext::default(),
        &goal,
        &[],
        &[],
        &proof,
        &mut Budget::default(),
    )
    .unwrap();
}

#[test]
fn mismatched_decrement_and_reversed_goal_still_reject() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
    let (context, assumptions, mut proof) = fixture(integer, literal(5), literal(1));
    let Proposition::LessThan(left, right) = &proof.conclusion else {
        unreachable!()
    };
    let reversed = Proposition::LessThan(right.clone(), left.clone());
    assert!(
        verify_bounded_certificate(
            &context,
            &reversed,
            &assumptions,
            &[],
            &ProofNode {
                conclusion: reversed.clone(),
                rule: proof.rule.clone()
            },
            &mut Budget::default()
        )
        .is_err()
    );
    let ProofRule::IntegerSubtractOrder { positive, .. } = &mut proof.rule else {
        unreachable!()
    };
    positive.conclusion = Proposition::LessThan(literal(0), literal(2));
    let changed = [assumptions[0].clone(), positive.conclusion.clone()];
    assert!(
        verify_bounded_certificate(
            &context,
            &proof.conclusion,
            &changed,
            &[],
            &proof,
            &mut Budget::default()
        )
        .is_err()
    );
}

#[test]
fn compound_and_value_equalities_compose_by_checked_identity_elimination() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar);
    let compound = ScalarTerm::exact_integer_subtract(integer, value(2), value(3)).unwrap();
    let first = Proposition::Equal(compound.clone(), value(1));
    let second = Proposition::Equal(value(1), value(2));
    let goal = Proposition::Equal(compound, value(2));
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: first.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: second.clone(),
                rule: ProofRule::Assumption { index: 1 },
            }),
        },
    };
    let context = PropositionContext::from_value_types(
        (1..=3).map(|identity| (ValueId::new(identity).unwrap(), scalar)),
    )
    .unwrap();
    let assumptions = [first, second];
    let parameters = BTreeSet::new();
    let mut elaboration =
        Elaboration::new(&context, &goal, &assumptions, &[], &parameters).unwrap();
    elaboration.node(&proof).unwrap();
    assert!(elaboration.denotation.rule_axioms.is_empty());
    assert!(
        elaboration.denotation.subtraction.laws.is_empty(),
        "composition requires only Id elimination, not arithmetic laws"
    );
    verify_bounded_certificate(
        &context,
        &goal,
        &assumptions,
        &[],
        &proof,
        &mut Budget::default(),
    )
    .unwrap();
}

#[test]
fn compositional_denotation_does_not_expand_bounded_premise_matching() {
    use semantic_vocabulary::IntegerMathTerm;

    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar);
    let (context, assumptions, _) = fixture(integer, value(2), value(3));
    let mathematical = |identity| IntegerMathTerm::MathValue {
        source_type: integer,
        value: ValueId::new(identity).unwrap(),
    };
    let goal = Proposition::IntegerMathEqual(
        mathematical(1),
        IntegerMathTerm::Subtract(Box::new(mathematical(2)), Box::new(mathematical(3))),
    );
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Assumption { index: 0 },
    };
    assert!(
        verify_bounded_certificate(
            &context,
            &goal,
            &assumptions,
            &[],
            &proof,
            &mut Budget::default()
        )
        .is_err()
    );
}
