use super::super::Elaboration;
use crate::{Budget, ProofNode, ProofRule, verify_mathematical_certificate};
use semantic_vocabulary::{
    IntegerSign, IntegerType, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};
use std::collections::BTreeSet;

fn integer() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

fn value(position: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(position).unwrap(),
        ScalarType::Integer(integer()),
    )
}

fn sum(left: u64, right: u64) -> ScalarTerm {
    ScalarTerm::exact_integer_add(integer(), value(left), value(right)).unwrap()
}

fn check(premise: Proposition, goal: Proposition, equations: &[(u64, u64)]) {
    let context = PropositionContext::from_value_types((1..=8).map(|position| {
        (
            ValueId::new(position).unwrap(),
            ScalarType::Integer(integer()),
        )
    }))
    .unwrap();
    let assumptions = [premise.clone()];
    let axioms: Vec<_> = equations
        .iter()
        .map(|&(left, right)| Proposition::Equal(value(left), value(right)))
        .collect();
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(ProofNode {
                conclusion: premise,
                rule: ProofRule::Assumption { index: 0 },
            }),
            equalities: axioms
                .iter()
                .enumerate()
                .map(|(index, conclusion)| ProofNode {
                    conclusion: conclusion.clone(),
                    rule: ProofRule::SemanticAxiom { index },
                })
                .collect(),
        },
    };
    let parameters = BTreeSet::new();
    let mut elaboration =
        Elaboration::new(&context, &goal, &assumptions, &axioms, &parameters).unwrap();
    let evidence = elaboration.node(&proof).unwrap();
    assert!(
        elaboration.denotation.rule_axioms.is_empty(),
        "substitution must derive its conclusion"
    );
    let mut denoted = elaboration.finish(evidence);
    verify_mathematical_certificate(
        &mut denoted.arena,
        &denoted.certificate,
        &mut Budget::default(),
    )
    .unwrap();
    let mut missing = axioms.clone();
    missing[0] = Proposition::Truth;
    let mut invalid =
        Elaboration::new(&context, &goal, &assumptions, &missing, &parameters).unwrap();
    assert!(
        invalid.node(&proof).is_err(),
        "missing cited equation must reject"
    );
}

#[test]
fn several_equations_transport_both_sides_of_nested_arithmetic() {
    check(
        Proposition::LessOrEqual(sum(1, 2), value(3)),
        Proposition::LessOrEqual(sum(4, 5), value(6)),
        &[(4, 1), (5, 2), (6, 3)],
    );
}

#[test]
fn dependent_equations_expand_in_either_roster_order() {
    for equations in [[(4, 5), (5, 1)], [(5, 1), (4, 5)]] {
        check(
            Proposition::LessOrEqual(sum(5, 2), value(3)),
            Proposition::LessOrEqual(sum(4, 2), value(3)),
            &equations,
        );
    }
}

#[test]
fn reversed_transport_preserves_unrelated_equal_occurrences() {
    check(
        Proposition::LessOrEqual(sum(1, 1), value(3)),
        Proposition::LessOrEqual(sum(4, 1), value(3)),
        &[(4, 1)],
    );
}

#[test]
fn transport_preserves_conjunction_implication_and_tag_binders() {
    let wrap = |left: u64, right: u64| {
        let order = Proposition::LessOrEqual(value(left), value(right));
        Proposition::Implication {
            premise: Box::new(order.clone()),
            conclusion: Box::new(Proposition::Conjunction(vec![
                order.clone(),
                Proposition::Disjunction(vec![order, Proposition::Falsehood]),
            ])),
        }
    };
    check(wrap(1, 2), wrap(4, 5), &[(4, 1), (5, 2)]);
}

#[test]
fn canonical_identity_orientation_uses_checked_symmetry() {
    check(
        Proposition::Equal(value(1), value(2)),
        Proposition::Equal(value(4), value(5)),
        &[(4, 2), (5, 1)],
    );
}

fn equal_value(left: u64, right: u64) -> Proposition {
    Proposition::Equal(value(left), value(right))
}

/// Canonical `IntegerMathEqual` ordering re-orients `Id` endpoints once an
/// equation rewrites a numeral out of canonical position; nested inside a
/// connective, the licensed pair then differs by a reversed `Id` at that
/// position — which is a `sym` `J`, not a rule-instance axiom.
#[test]
fn reversed_identity_nested_in_implication_premise_orients_by_j() {
    check(
        Proposition::Implication {
            premise: Box::new(equal_value(1, 2)),
            conclusion: Box::new(equal_value(1, 3)),
        },
        Proposition::Implication {
            premise: Box::new(equal_value(4, 2)),
            conclusion: Box::new(equal_value(4, 3)),
        },
        &[(4, 1)],
    );
}

#[test]
fn reversed_identity_nested_in_disjunction_orients_by_j() {
    check(
        Proposition::Disjunction(vec![equal_value(1, 2), equal_value(1, 3)]),
        Proposition::Disjunction(vec![equal_value(4, 2), equal_value(4, 3)]),
        &[(4, 1)],
    );
}

#[test]
fn reversed_identity_nested_in_negation_orients_by_j() {
    check(
        Proposition::Implication {
            premise: Box::new(equal_value(1, 2)),
            conclusion: Box::new(Proposition::Falsehood),
        },
        Proposition::Implication {
            premise: Box::new(equal_value(4, 2)),
            conclusion: Box::new(Proposition::Falsehood),
        },
        &[(4, 1)],
    );
}

#[test]
fn reversed_identity_nested_in_conjunction_orients_by_j() {
    let context = |identity: Proposition| {
        Proposition::Conjunction(vec![Proposition::LessOrEqual(value(7), value(8)), identity])
    };
    check(
        context(equal_value(1, 2)),
        context(equal_value(4, 2)),
        &[(4, 1)],
    );
}

/// Transport that would have to swap the *unrewritten* endpoint of a nested
/// identity is not licensed: the shared relation rejects it before the
/// denotation is consulted, and the denotation never gets to name it an
/// axiom either.
#[test]
fn nested_identity_requiring_unlicensed_reversal_rejects() {
    let premise = Proposition::Implication {
        premise: Box::new(equal_value(1, 2)),
        conclusion: Box::new(equal_value(1, 3)),
    };
    let goal = Proposition::Implication {
        premise: Box::new(equal_value(2, 4)),
        conclusion: Box::new(equal_value(2, 3)),
    };
    let axioms = [equal_value(4, 1)];
    let context = PropositionContext::from_value_types((1..=4).map(|position| {
        (
            ValueId::new(position).unwrap(),
            ScalarType::Integer(integer()),
        )
    }))
    .unwrap();
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            equalities: axioms
                .iter()
                .enumerate()
                .map(|(index, conclusion)| ProofNode {
                    conclusion: conclusion.clone(),
                    rule: ProofRule::SemanticAxiom { index },
                })
                .collect(),
        },
    };
    let assumptions = [premise];
    let parameters = BTreeSet::new();
    let mut elaboration =
        Elaboration::new(&context, &goal, &assumptions, &axioms, &parameters).unwrap();
    assert!(elaboration.node(&proof).is_err());
}
