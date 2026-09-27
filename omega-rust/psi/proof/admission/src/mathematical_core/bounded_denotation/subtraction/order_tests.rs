//! `IntegerAddOrder` and `IntegerSubtractAntitone`: an open instance
//! elaborates through the existing core laws with no rule-instance axiom, a
//! closed one keeps the checked rule-instance route, and every premise,
//! orientation and minuend is load-bearing.

use std::collections::BTreeSet;

use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

use super::super::Elaboration;
use crate::{
    Budget, ProofNode, ProofRule, Term, verify_bounded_certificate, verify_mathematical_certificate,
};

fn value(integer: IntegerType, identity: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(integer),
    )
}

fn zero(integer: IntegerType) -> ScalarTerm {
    ScalarTerm::integer(
        integer,
        match integer.sign() {
            IntegerSign::Signed => IntegerValue::Signed(0),
            IntegerSign::Unsigned => IntegerValue::Unsigned(0),
        },
    )
    .unwrap()
}

fn context(integer: IntegerType) -> PropositionContext {
    PropositionContext::from_value_types((1..=6).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer),
        )
    }))
    .unwrap()
}

fn cited(conclusion: Proposition, index: usize) -> Box<ProofNode> {
    Box::new(ProofNode {
        conclusion,
        rule: ProofRule::Assumption { index },
    })
}

/// `result = original + increment, 0 < increment ⊢ original < result`.
fn add_order(
    integer: IntegerType,
    original: ScalarTerm,
    increment: ScalarTerm,
) -> (Vec<Proposition>, ProofNode) {
    let result = value(integer, 1);
    let sum = Proposition::Equal(
        result.clone(),
        ScalarTerm::exact_integer_add(integer, original.clone(), increment.clone()).unwrap(),
    );
    let positive = Proposition::LessThan(zero(integer), increment);
    let proof = ProofNode {
        conclusion: Proposition::LessThan(original, result),
        rule: ProofRule::IntegerAddOrder {
            sum: cited(sum.clone(), 0),
            positive: cited(positive.clone(), 1),
        },
    };
    (vec![sum, positive], proof)
}

/// `a = m - large, b = m - small, small < large ⊢ a < b`.
fn antitone(
    integer: IntegerType,
    minuend: ScalarTerm,
    large: ScalarTerm,
    small: ScalarTerm,
) -> (Vec<Proposition>, ProofNode) {
    let (smaller, larger) = (value(integer, 1), value(integer, 2));
    let low = Proposition::Equal(
        smaller.clone(),
        ScalarTerm::exact_integer_subtract(integer, minuend.clone(), large.clone()).unwrap(),
    );
    let high = Proposition::Equal(
        larger.clone(),
        ScalarTerm::exact_integer_subtract(integer, minuend, small.clone()).unwrap(),
    );
    let order = Proposition::LessThan(small, large);
    let proof = ProofNode {
        conclusion: Proposition::LessThan(smaller, larger),
        rule: ProofRule::IntegerSubtractAntitone {
            smaller: cited(low.clone(), 0),
            larger: cited(high.clone(), 1),
            order: cited(order.clone(), 2),
        },
    };
    (vec![low, high, order], proof)
}

/// The certificate checks, its mathematical term re-checks and a forged
/// term does not, and dropping the last premise rejects. Returns whether
/// the elaboration needed a rule-instance axiom.
fn checked(integer: IntegerType, assumptions: &[Proposition], proof: &ProofNode) -> bool {
    let context = context(integer);
    let parameters = BTreeSet::new();
    let mut elaboration =
        Elaboration::new(&context, &proof.conclusion, assumptions, &[], &parameters).unwrap();
    elaboration.node(proof).unwrap();
    let rule_instance = !elaboration.denotation.rule_axioms.is_empty();
    let mut denoted = verify_bounded_certificate(
        &context,
        &proof.conclusion,
        assumptions,
        &[],
        proof,
        &mut Budget::default(),
    )
    .unwrap();
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
            &assumptions[..assumptions.len() - 1],
            &[],
            proof,
            &mut Budget::default()
        )
        .is_err()
    );
    rule_instance
}

fn rejects(integer: IntegerType, assumptions: &[Proposition], proof: &ProofNode) {
    assert!(
        verify_bounded_certificate(
            &context(integer),
            &proof.conclusion,
            assumptions,
            &[],
            proof,
            &mut Budget::default()
        )
        .is_err()
    );
}

#[test]
fn open_addition_order_derives_from_the_inverse_and_antitone_laws() {
    for integer in [
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        IntegerType::new(IntegerSign::Signed, 32).unwrap(),
    ] {
        let one = ScalarTerm::integer(
            integer,
            match integer.sign() {
                IntegerSign::Signed => IntegerValue::Signed(1),
                IntegerSign::Unsigned => IntegerValue::Unsigned(1),
            },
        )
        .unwrap();
        for increment in [value(integer, 3), one] {
            let (assumptions, proof) = add_order(integer, value(integer, 2), increment);
            assert!(!checked(integer, &assumptions, &proof));
        }
    }
}

#[test]
fn closed_addition_order_keeps_the_checked_rule_instance() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
    let (assumptions, proof) = add_order(integer, literal(5), literal(3));
    assert!(checked(integer, &assumptions, &proof));
}

#[test]
fn addition_order_rejects_reversal_nonstrict_positivity_and_another_increment() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let (assumptions, proof) = add_order(integer, value(integer, 2), value(integer, 3));
    let Proposition::LessThan(original, result) = &proof.conclusion else {
        unreachable!()
    };
    let mut reversed = proof.clone();
    reversed.conclusion = Proposition::LessThan(result.clone(), original.clone());
    rejects(integer, &assumptions, &reversed);

    let mut nonstrict = assumptions.clone();
    nonstrict[1] = Proposition::LessOrEqual(zero(integer), value(integer, 3));
    let mut weakened = proof.clone();
    let ProofRule::IntegerAddOrder { positive, .. } = &mut weakened.rule else {
        unreachable!()
    };
    positive.conclusion = nonstrict[1].clone();
    rejects(integer, &nonstrict, &weakened);

    let mut other = assumptions.clone();
    other[1] = Proposition::LessThan(zero(integer), value(integer, 4));
    let mut redirected = proof.clone();
    let ProofRule::IntegerAddOrder { positive, .. } = &mut redirected.rule else {
        unreachable!()
    };
    positive.conclusion = other[1].clone();
    rejects(integer, &other, &redirected);
}

#[test]
fn open_subtraction_antitonicity_derives_from_the_antitone_law() {
    for integer in [
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        IntegerType::new(IntegerSign::Signed, 16).unwrap(),
    ] {
        let (assumptions, proof) = antitone(
            integer,
            value(integer, 3),
            value(integer, 4),
            value(integer, 5),
        );
        assert!(!checked(integer, &assumptions, &proof));
    }
}

#[test]
fn closed_subtraction_antitonicity_keeps_the_checked_rule_instance() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
    let (assumptions, proof) = antitone(integer, literal(9), literal(4), literal(2));
    assert!(checked(integer, &assumptions, &proof));
}

#[test]
fn subtraction_antitonicity_rejects_another_minuend_and_a_reversed_order() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let (assumptions, proof) = antitone(
        integer,
        value(integer, 3),
        value(integer, 4),
        value(integer, 5),
    );
    let mut foreign = assumptions.clone();
    foreign[1] = Proposition::Equal(
        value(integer, 2),
        ScalarTerm::exact_integer_subtract(integer, value(integer, 6), value(integer, 5)).unwrap(),
    );
    let mut rerooted = proof.clone();
    let ProofRule::IntegerSubtractAntitone { larger, .. } = &mut rerooted.rule else {
        unreachable!()
    };
    larger.conclusion = foreign[1].clone();
    rejects(integer, &foreign, &rerooted);

    let mut backwards = assumptions.clone();
    backwards[2] = Proposition::LessThan(value(integer, 4), value(integer, 5));
    let mut flipped = proof.clone();
    let ProofRule::IntegerSubtractAntitone { order, .. } = &mut flipped.rule else {
        unreachable!()
    };
    order.conclusion = backwards[2].clone();
    rejects(integer, &backwards, &flipped);

    let Proposition::LessThan(smaller, larger) = &proof.conclusion else {
        unreachable!()
    };
    let mut reversed = proof.clone();
    reversed.conclusion = Proposition::LessThan(larger.clone(), smaller.clone());
    rejects(integer, &assumptions, &reversed);
}
