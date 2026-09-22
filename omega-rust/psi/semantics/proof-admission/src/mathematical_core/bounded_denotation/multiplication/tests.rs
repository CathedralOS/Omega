use std::collections::BTreeSet;

use super::super::Elaboration;
use super::Law;
use crate::{Budget, IntegerAffineWitness, ProofNode, ProofRule, verify_bounded_certificate};
use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};

fn mathematical(term: &ScalarTerm) -> IntegerMathTerm {
    match term {
        ScalarTerm::Value {
            id,
            scalar_type: ScalarType::Integer(source_type),
        } => IntegerMathTerm::MathValue {
            source_type: *source_type,
            value: *id,
        },
        ScalarTerm::Integer { value, .. } => IntegerMathTerm::literal(*value),
        _ => unreachable!(),
    }
}

/// One checked correlated-multiply bound certificate: the quotient
/// `root = endpoint / right` correlates `left · right` with the carrier
/// endpoint, and the evidence conjunction carries the divisor sign and
/// the root bound in the checked order.
struct Fixture {
    context: PropositionContext,
    axioms: Vec<Proposition>,
    premise: Proposition,
    goal: Proposition,
    proof: ProofNode,
}

fn fixture(
    integer: IntegerType,
    endpoint: IntegerValue,
    lower: bool,
    positive: bool,
    landed: bool,
    divide_root: bool,
) -> Fixture {
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let literal = |value| ScalarTerm::integer(integer, value).unwrap();
    let left = value(1);
    let right = value(2);
    let quotient = value(3);
    let endpoint_term = if landed { value(4) } else { literal(endpoint) };
    let context = PropositionContext::from_value_types(
        (1..=4).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let divide =
        ScalarTerm::exact_integer_divide(integer, endpoint_term.clone(), right.clone()).unwrap();
    let mut axioms = Vec::new();
    let (root, definition_axioms, literal_axioms) = if divide_root {
        (divide.clone(), Vec::new(), Vec::new())
    } else {
        if landed {
            axioms.push(Proposition::Equal(endpoint_term.clone(), literal(endpoint)));
        }
        let definition_index = axioms.len();
        axioms.push(Proposition::Equal(quotient.clone(), divide));
        (quotient, vec![definition_index], vec![landed.then_some(0)])
    };
    let one = literal(match integer.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    });
    let sign = if positive {
        Proposition::LessOrEqual(one, right.clone())
    } else {
        Proposition::LessOrEqual(right.clone(), literal(IntegerValue::Signed(-2)))
    };
    let bound = if lower == positive {
        Proposition::LessOrEqual(root.clone(), left.clone())
    } else {
        Proposition::LessOrEqual(left.clone(), root.clone())
    };
    let premise = Proposition::Conjunction(vec![sign, bound]);
    let product = IntegerMathTerm::Multiply(
        Box::new(mathematical(&left)),
        Box::new(mathematical(&right)),
    );
    let bound_literal = IntegerMathTerm::literal(endpoint);
    let goal = if lower {
        Proposition::IntegerMathLessOrEqual(bound_literal, product)
    } else {
        Proposition::IntegerMathLessOrEqual(product, bound_literal)
    };
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            witness: IntegerAffineWitness {
                root,
                target: ScalarTerm::exact_integer_multiply(integer, left, right).unwrap(),
                definition_axioms,
                literal_axioms,
            },
        },
    };
    Fixture {
        context,
        axioms,
        premise,
        goal,
        proof,
    }
}

fn check(fixture: &Fixture, laws: &[Law]) {
    verify_bounded_certificate(
        &fixture.context,
        &fixture.goal,
        std::slice::from_ref(&fixture.premise),
        &fixture.axioms,
        &fixture.proof,
        &mut Budget::default(),
    )
    .unwrap();
    let parameters = BTreeSet::new();
    let mut elaboration = Elaboration::new(
        &fixture.context,
        &fixture.goal,
        std::slice::from_ref(&fixture.premise),
        &fixture.axioms,
        &parameters,
    )
    .unwrap();
    let evidence = elaboration.node(&fixture.proof).unwrap();
    assert!(elaboration.denotation.rule_axioms.is_empty());
    assert!(elaboration.denotation.decisions.is_empty());
    for law in laws {
        assert!(
            elaboration.denotation.multiplication.laws.contains_key(law),
            "missing {law:?}"
        );
    }
    // The evidence the elaboration produced is a real derivation, not a
    // constant naming the conclusion.
    let term = elaboration.denotation.arena.get(evidence);
    assert!(!matches!(
        term,
        crate::mathematical_core::Term::Constant { .. }
    ));
}

/// `div(−128, r) ≤ l` with `1 ≤ r` proves `−128 ≤ l·r` over `i8` — the
/// monotone law lifts `div_app ≤ l'` and the positive-divisor residual
/// lands the minimum endpoint.
#[test]
fn correlated_multiply_positive_minimum() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let fixture = fixture(
        integer,
        IntegerValue::Signed(-128),
        true,
        true,
        false,
        false,
    );
    check(
        &fixture,
        &[Law::MonotonePositive, Law::DivideLowerPositive(integer)],
    );
}

/// `l ≤ div(−128, r)` with `r ≤ −2` proves `−128 ≤ l·r` over `i8` —
/// the antitone law reverses the bound and the negative-divisor
/// residual lands the minimum.
#[test]
fn correlated_multiply_negative_minimum() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let fixture = fixture(
        integer,
        IntegerValue::Signed(-128),
        true,
        false,
        false,
        false,
    );
    check(
        &fixture,
        &[Law::AntitoneNegative, Law::DivideLowerNegative(integer)],
    );
}

/// `l ≤ div(127, r)` with `1 ≤ r` proves `l·r ≤ 127` over `i8`.
#[test]
fn correlated_multiply_positive_maximum() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let fixture = fixture(
        integer,
        IntegerValue::Signed(127),
        false,
        true,
        false,
        false,
    );
    check(
        &fixture,
        &[Law::MonotonePositive, Law::DivideUpperPositive(integer)],
    );
}

/// `div(127, r) ≤ l` with `r ≤ −2` proves `l·r ≤ 127` over `i8`.
#[test]
fn correlated_multiply_negative_maximum() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let fixture = fixture(
        integer,
        IntegerValue::Signed(127),
        false,
        false,
        false,
        false,
    );
    check(
        &fixture,
        &[Law::AntitoneNegative, Law::DivideUpperNegative(integer)],
    );
}

/// A bare divide term as the witness root needs no cited definition —
/// the denoted `div_app` is already the bound's endpoint.
#[test]
fn correlated_multiply_divide_root_needs_no_definition() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let fixture = fixture(integer, IntegerValue::Signed(-128), true, true, false, true);
    check(
        &fixture,
        &[Law::MonotonePositive, Law::DivideLowerPositive(integer)],
    );
}

/// A landed endpoint value — `endpoint = v`, `v = min` cited separately —
/// substitutes to its numeral on the residual premise and on the goal's
/// bound endpoint.
#[test]
fn correlated_multiply_landed_endpoint_substitutes_both_sites() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let fixture = fixture(integer, IntegerValue::Signed(-128), true, true, true, false);
    check(
        &fixture,
        &[Law::MonotonePositive, Law::DivideLowerPositive(integer)],
    );
}

/// `0 = min` over `u8` exercises the endpoint-equality-free reflexive
/// numeral bound.
#[test]
fn correlated_multiply_unsigned_zero_endpoint() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let fixture = fixture(integer, IntegerValue::Unsigned(0), true, true, false, false);
    check(
        &fixture,
        &[Law::MonotonePositive, Law::DivideLowerPositive(integer)],
    );
}

/// The four-corner direct multiply bound is a different checked family —
/// its premise shape is outside the correlated vocabulary, so it keeps
/// the explicit per-instance rule axiom.
#[test]
fn direct_multiply_bound_keeps_the_instance_fallback() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let literal = |number| ScalarTerm::integer(integer, IntegerValue::Signed(number)).unwrap();
    let context = PropositionContext::from_value_types(
        (1..=2).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let left = value(1);
    let right = value(2);
    let premise = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(literal(-4), left.clone()),
        Proposition::LessOrEqual(left.clone(), literal(5)),
        Proposition::LessOrEqual(literal(-3), right.clone()),
        Proposition::LessOrEqual(right.clone(), literal(2)),
    ]);
    let product = IntegerMathTerm::Multiply(
        Box::new(mathematical(&left)),
        Box::new(mathematical(&right)),
    );
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::literal(IntegerValue::Signed(-15)),
        product,
    );
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            witness: IntegerAffineWitness {
                root: left,
                target: ScalarTerm::exact_integer_multiply(integer, value(1), right).unwrap(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        },
    };
    verify_bounded_certificate(
        &context,
        &goal,
        std::slice::from_ref(&premise),
        &[],
        &proof,
        &mut Budget::default(),
    )
    .unwrap();
    let parameters = BTreeSet::new();
    let mut elaboration = Elaboration::new(
        &context,
        &goal,
        std::slice::from_ref(&premise),
        &[],
        &parameters,
    )
    .unwrap();
    elaboration.node(&proof).unwrap();
    assert_eq!(elaboration.denotation.rule_axioms.len(), 1);
}

/// Mutating the checked premise rejects at the shared relation before
/// denotation — the derivation cannot restate a flipped bound.
#[test]
fn correlated_multiply_rejects_mutated_premise_and_goal() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let fixture = fixture(
        integer,
        IntegerValue::Signed(-128),
        true,
        true,
        false,
        false,
    );
    // The bound conjunct flipped: `l ≤ root` is the wrong orientation
    // for a positive-divisor minimum.
    let mut flipped = fixture.proof.clone();
    let ProofRule::IntegerAffineBound { root_bound, .. } = &mut flipped.rule else {
        unreachable!()
    };
    root_bound.conclusion = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(
            ScalarTerm::integer(integer, IntegerValue::Signed(1)).unwrap(),
            value(2),
        ),
        Proposition::LessOrEqual(value(1), value(3)),
    ]);
    let flipped_premise = root_bound.conclusion.clone();
    assert!(
        verify_bounded_certificate(
            &fixture.context,
            &fixture.goal,
            std::slice::from_ref(&flipped_premise),
            &fixture.axioms,
            &flipped,
            &mut Budget::default()
        )
        .is_err()
    );
    // A conclusion literal that is not the carrier endpoint mismatches
    // the checked map outright.
    let mut wrong_goal = fixture.proof.clone();
    wrong_goal.conclusion = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::literal(IntegerValue::Signed(-127)),
        IntegerMathTerm::Multiply(
            Box::new(mathematical(&value(1))),
            Box::new(mathematical(&value(2))),
        ),
    );
    assert!(
        verify_bounded_certificate(
            &fixture.context,
            &wrong_goal.conclusion,
            std::slice::from_ref(&fixture.premise),
            &fixture.axioms,
            &wrong_goal,
            &mut Budget::default()
        )
        .is_err()
    );
    // The sign conjunct alone is not the checked conjunction shape.
    let mut missing = fixture.proof.clone();
    let ProofRule::IntegerAffineBound { root_bound, .. } = &mut missing.rule else {
        unreachable!()
    };
    root_bound.conclusion = Proposition::LessOrEqual(value(3), value(1));
    let missing_premise = root_bound.conclusion.clone();
    assert!(
        verify_bounded_certificate(
            &fixture.context,
            &fixture.goal,
            std::slice::from_ref(&missing_premise),
            &fixture.axioms,
            &missing,
            &mut Budget::default()
        )
        .is_err()
    );
}
