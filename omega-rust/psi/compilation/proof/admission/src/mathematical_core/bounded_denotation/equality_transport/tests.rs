use super::super::Elaboration;
use crate::proof::assumption_proof;
use crate::{
    Budget, ProofNode, ProofRule, proof_node, value_context, verify_mathematical_certificate,
};
use semantic_vocabulary::{IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType, ValueId};
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
    let context = value_context(ScalarType::Integer(integer()), 8);
    let assumptions = [premise.clone()];
    let axioms: Vec<_> = equations
        .iter()
        .map(|&(left, right)| Proposition::Equal(value(left), value(right)))
        .collect();
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(assumption_proof(premise, 0)),
            equalities: axioms
                .iter()
                .enumerate()
                .map(|(index, conclusion)| proof_node!(conclusion.clone() => ProofRule::SemanticAxiom { index }))
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
    let context = value_context(ScalarType::Integer(integer()), 4);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(assumption_proof(premise.clone(), 0)),
            equalities: axioms
                .iter()
                .enumerate()
                .map(|(index, conclusion)| proof_node!(conclusion.clone() => ProofRule::SemanticAxiom { index }))
                .collect(),
        },
    };
    let assumptions = [premise];
    let parameters = BTreeSet::new();
    let mut elaboration =
        Elaboration::new(&context, &goal, &assumptions, &axioms, &parameters).unwrap();
    assert!(elaboration.node(&proof).is_err());
}

#[test]
fn deep_oriented_refutation_uses_default_stack_and_keeps_true_premises_open() {
    std::thread::spawn(|| {
        use super::Denotation;
        use crate::mathematical_core::{Context, Term, infer_sort};
        for (depth, refutable) in [(8192, true), (32, true), (32, false)] {
            let mut d = Denotation::new();
            let truth = d.denote(&Proposition::Truth).unwrap();
            let goal = d.denote(&Proposition::Falsehood).unwrap();
            let mut premise = if refutable { goal } else { truth };
            for _ in 0..depth {
                premise = d.arena.insert(Term::Sigma {
                    domain: premise,
                    codomain: truth,
                });
            }
            infer_sort(
                &mut d.arena,
                &Context::empty(),
                premise,
                &mut Budget::new(2_000_000),
            )
            .expect("the complete premise is independently formed");
            let evidence = d.arena.insert(Term::Variable(0));
            eprintln!(
                "{depth}-node premise formed; begin oriented refutation refutable={refutable}"
            );
            let result = d.oriented_evidence(premise, goal, evidence).unwrap();
            eprintln!("oriented refutation returned");
            if refutable {
                let term = result.expect("the exact false conjunct refutes under its premise");
                let mut pending = vec![term];
                let mut visited = std::collections::HashSet::new();
                let mut projections = 0;
                while let Some(handle) = pending.pop() {
                    if !visited.insert(handle) {
                        continue;
                    }
                    let node = d.arena.get(handle);
                    if matches!(node, Term::Fst { .. }) {
                        projections += 1;
                    }
                    pending.extend(node.subterms().iter().map(|(child, _)| child));
                }
                assert_eq!(
                    projections, depth,
                    "the evidence spends the actual deepest premise"
                );
                if depth == 32 {
                    let certificate =
                        crate::MathematicalCertificate::closed(vec![premise], term, goal);
                    verify_mathematical_certificate(
                        &mut d.arena,
                        &certificate,
                        &mut Budget::new(2_000_000),
                    )
                    .expect("the independent kernel checks the full bounded refutation");
                }
            } else {
                assert!(
                    result.is_none(),
                    "closed true premises do not prove Falsehood"
                );
            }
            assert!(
                d.declarations.is_empty(),
                "the search does not invent a decision axiom"
            );
        }
    })
    .join()
    .expect("oriented evidence construction stays on a default stack");
}

#[test]
fn oriented_search_shares_allowance_across_failed_alternatives() {
    use super::Denotation;
    use crate::mathematical_core::Term;
    for allowance in 0..=3 {
        let mut d = Denotation::new();
        let first = equal_value(1, 2);
        let last = equal_value(1, 3);
        let premise = d
            .denote(&Proposition::Conjunction(vec![first, last.clone()]))
            .unwrap();
        let goal = d.denote(&last).unwrap();
        let evidence = d.arena.insert(Term::Variable(0));
        let declarations = d.declarations.len();
        let mut remaining = allowance;
        let result = d
            .oriented_coercion(premise, goal, evidence, &mut remaining)
            .unwrap();
        assert_eq!(
            remaining, 0,
            "the root and failed first alternative consume the same allowance"
        );
        if allowance == 3 {
            let term = result
                .expect("the second projection is reached after the failed first alternative");
            assert!(matches!(d.arena.get(term), Term::Snd { pair } if pair == evidence));
            let certificate = crate::MathematicalCertificate::with_signature(
                d.declarations.clone(),
                vec![premise],
                term,
                goal,
            );
            verify_mathematical_certificate(
                &mut d.arena,
                &certificate,
                &mut Budget::new(2_000_000),
            )
            .unwrap();
        } else {
            assert!(
                result.is_none(),
                "a declined first alternative cannot refund its allowance"
            );
        }
        assert_eq!(declarations, d.declarations.len());
        assert!(
            d.rule_axioms.is_empty(),
            "exhaustion cannot invent a decision axiom"
        );
    }
}
