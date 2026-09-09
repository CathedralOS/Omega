use proof_admission::{AcceptedPremise, accept_certificate};
use semantic_vocabulary::{IntegerSign, IntegerType, PropositionContext, ValueId};

use super::super::{MAXIMUM_SEARCH_STEPS, establishes};
use super::*;

fn integer() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).unwrap()
}

fn value(identity: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(integer()),
    )
}

fn order(left: u64, right: u64, strict: bool) -> Proposition {
    if strict {
        Proposition::LessThan(value(left), value(right))
    } else {
        Proposition::LessOrEqual(value(left), value(right))
    }
}

fn context() -> PropositionContext {
    PropositionContext::from_value_types((1..=6).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer()),
        )
    }))
    .unwrap()
}

#[test]
fn strict_and_nonstrict_chains_keep_both_premise_namespaces() {
    for first_strict in [false, true] {
        for second_strict in [false, true] {
            let requirements = [order(1, 2, first_strict)];
            let axioms = [order(2, 3, second_strict)];
            for strict_goal in [false, true] {
                let goal = order(1, 3, strict_goal);
                let mut remaining = MAXIMUM_SEARCH_STEPS;
                let proof = prove(&goal, &requirements, &axioms, &mut remaining, 0);
                if strict_goal && !first_strict && !second_strict {
                    assert!(
                        proof.is_none(),
                        "two inclusive premises cannot establish strictness"
                    );
                    continue;
                }
                let accepted =
                    accept_certificate(&context(), &goal, &requirements, &axioms, &proof.unwrap())
                        .unwrap();
                assert_eq!(
                    accepted.assumptions,
                    vec![AcceptedPremise {
                        index: 0,
                        proposition: requirements[0].clone(),
                    }]
                );
                assert_eq!(
                    accepted.semantic_axioms,
                    vec![AcceptedPremise {
                        index: 0,
                        proposition: axioms[0].clone(),
                    }]
                );
            }
        }
    }
}

#[test]
fn conjunctive_long_chains_and_parallel_strict_paths_are_searchable() {
    let goal = order(1, 5, true);
    let requirements = [Proposition::Conjunction(vec![
        order(1, 2, false),
        Proposition::Conjunction(vec![order(2, 3, false), order(3, 4, true)]),
    ])];
    assert!(establishes(
        &context(),
        &goal,
        &requirements,
        &[order(4, 5, false)]
    ));
    // Reaching 2 inclusively first must not hide the later strict path to 2.
    let facts = [order(1, 2, false), order(1, 2, true), order(2, 5, false)];
    assert!(establishes(&context(), &goal, &[], &facts));
}

#[test]
fn disconnected_reversed_and_conditional_edges_cannot_establish_a_chain() {
    let goal = order(1, 3, true);
    let first = order(1, 2, true);
    for other in [order(3, 2, false), order(4, 3, false), order(2, 4, false)] {
        assert!(!establishes(
            &context(),
            &goal,
            &[],
            &[first.clone(), other]
        ));
    }
    for alternative in [
        Proposition::Disjunction(vec![first.clone(), order(2, 3, false)]),
        Proposition::Implication {
            premise: Box::new(first.clone()),
            conclusion: Box::new(order(2, 3, false)),
        },
    ] {
        assert!(!establishes(&context(), &goal, &[], &[alternative]));
    }
    // Unrelated cycles do not grant a missing connection or loop indefinitely.
    let cyclic = [
        first,
        order(2, 1, false),
        order(4, 3, false),
        order(3, 4, false),
    ];
    assert!(!establishes(&context(), &goal, &[], &cyclic));
}

#[test]
fn search_and_certificate_construction_share_the_existing_budget() {
    let goal = order(1, 3, true);
    let facts = [order(1, 2, true), order(2, 3, false)];
    let mut remaining = 1;
    assert!(prove(&goal, &[], &facts, &mut remaining, 0).is_none());
    assert_eq!(remaining, 0);
    let mut remaining = MAXIMUM_SEARCH_STEPS;
    assert!(prove(&goal, &[], &facts, &mut remaining, 64).is_none());
}
