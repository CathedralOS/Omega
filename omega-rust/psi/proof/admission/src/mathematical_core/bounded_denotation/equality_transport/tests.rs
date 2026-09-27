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

fn equal_value(left: u64, right: u64) -> Proposition {
    Proposition::Equal(value(left), value(right))
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
