use std::cell::Cell;

use super::*;
use psi_core::{
    IntegerSign, IntegerType, IntegerValue, PropositionContext, PropositionId, ScalarType,
};
use psi_proof_admission::{ProofRule, accept_certificate};

fn integer_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 8).unwrap()
}

fn value(index: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(index).unwrap(),
        ScalarType::Integer(integer_type()),
    )
}

fn integer(value: i128) -> ScalarTerm {
    ScalarTerm::integer(integer_type(), IntegerValue::Signed(value)).unwrap()
}

fn outside(index: u64, magnitude: i128) -> Proposition {
    Proposition::Disjunction(vec![
        Proposition::LessOrEqual(value(index), integer(-magnitude)),
        Proposition::LessOrEqual(integer(magnitude), value(index)),
    ])
}

#[test]
fn disconnected_cases_do_not_multiply_search_even_with_shared_literal_aliases() {
    for shared_literal in [false, true] {
        let zero = if shared_literal {
            value(1001)
        } else {
            integer(0)
        };
        let mut facts = (1..=64)
            .map(|index| {
                Proposition::Disjunction(vec![
                    Proposition::LessThan(value(index), zero.clone()),
                    Proposition::LessThan(zero.clone(), value(index)),
                ])
            })
            .collect::<Vec<_>>();
        if shared_literal {
            // Reverse order requires the exact alias closure, not one scan.
            facts.push(Proposition::Equal(value(1001), value(1002)));
            facts.push(Proposition::Equal(integer(0), value(1002)));
        }
        let goal = Proposition::LessThan(zero, value(999));
        assert!(connected_cases(&goal, &[], &facts).is_empty());
        let calls = Cell::new(0);
        assert!(
            super::super::prove(&goal, &[], &facts, |_| {
                calls.set(calls.get() + 1);
                None
            })
            .is_none()
        );
        assert_eq!(calls.get(), 1, "no disconnected alternative is explored");
    }
}

#[test]
fn aliases_and_coupled_facts_connect_cases_transitively() {
    let goal = Proposition::LessOrEqual(integer(1), value(1));
    let relevant = outside(4, 2);
    let unrelated = outside(5, 2);
    let facts = [
        relevant.clone(),
        unrelated,
        Proposition::Equal(value(3), value(4)),
        Proposition::LessOrEqual(value(2), value(3)),
        Proposition::Equal(value(1), value(2)),
    ];
    let selected = connected_cases(&goal, &[], &facts);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].1, &relevant);
}

#[test]
fn case_search_preserves_jointly_required_cases_and_kernel_acceptance() {
    let context = PropositionContext::from_value_types((1..=2).map(|index| {
        (
            ValueId::new(index).unwrap(),
            ScalarType::Integer(integer_type()),
        )
    }))
    .unwrap();
    let goal = Proposition::Conjunction(vec![outside(1, 1), outside(2, 1)]);
    let facts = [outside(1, 2), outside(2, 2)];
    let calls = Cell::new(0);
    let proof = super::super::prove(&goal, &[], &facts, |assumptions| {
        calls.set(calls.get() + 1);
        super::super::super::build_without_cases(
            &context,
            &goal,
            assumptions,
            &facts,
            &BTreeSet::new(),
        )
    })
    .expect("both case assumptions remain available jointly");
    assert_eq!(calls.get(), 7, "root, two single cases, four joint cases");
    assert!(matches!(
        proof.rule,
        ProofRule::DisjunctionElimination { .. }
    ));
    accept_certificate(&context, &goal, &[], &facts, &proof).unwrap();
}

#[test]
fn opaque_and_value_free_cases_are_retained_conservatively() {
    let opaque = Proposition::Disjunction(vec![
        Proposition::Atom(PropositionId::new(1).unwrap()),
        Proposition::Truth,
    ]);
    let value_free = Proposition::Disjunction(vec![Proposition::Truth, Proposition::Falsehood]);
    let facts = [opaque, value_free, outside(2, 1)];
    let goal = Proposition::LessThan(integer(0), value(1));
    assert_eq!(connected_cases(&goal, &[], &facts).len(), 2);
    assert_eq!(connected_cases(&Proposition::Truth, &[], &facts).len(), 3);
}

#[test]
fn conditional_literal_equalities_are_not_treated_as_known_constants() {
    let goal = Proposition::LessThan(value(1), value(2));
    let facts = [
        Proposition::Disjunction(vec![
            Proposition::Equal(value(2), integer(0)),
            Proposition::Equal(value(2), integer(1)),
        ]),
        Proposition::Disjunction(vec![
            Proposition::LessThan(value(2), value(3)),
            Proposition::LessThan(value(3), value(2)),
        ]),
    ];
    assert_eq!(connected_cases(&goal, &[], &facts).len(), 2);
}
