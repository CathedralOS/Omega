use std::cell::Cell;

use super::*;
use proof_admission::{ProofRule, accept_certificate};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, PropositionContext, PropositionId, ScalarType,
};

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
    assert_eq!(selected[0].proposition, &relevant);
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

#[test]
fn conjunction_packaging_does_not_connect_independent_cases() {
    let cases = (1..=64)
        .map(|index| {
            Proposition::Disjunction(vec![
                Proposition::LessThan(value(index), value(1001)),
                Proposition::LessThan(value(1001), value(index)),
            ])
        })
        .collect::<Vec<_>>();
    let facts = [Proposition::Conjunction(vec![
        Proposition::Conjunction(cases.clone()),
        Proposition::Equal(value(1001), value(1002)),
        Proposition::Equal(value(1002), integer(0)),
    ])];
    let goal = Proposition::LessThan(integer(0), value(1));
    let selected = connected_cases(&goal, &[], &facts);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].proposition, &cases[0]);
    assert!(
        connected_cases(&Proposition::LessThan(integer(0), value(999)), &[], &facts,).is_empty()
    );
}

#[test]
fn nested_boolean_cases_replay_exact_projections_and_actual_aliases() {
    let context = PropositionContext::from_value_types(
        (1..=4).map(|index| (ValueId::new(index).unwrap(), ScalarType::Boolean)),
    )
    .unwrap();
    let boolean = |index| ScalarTerm::value(ValueId::new(index).unwrap(), ScalarType::Boolean);
    let positive = |index| Proposition::Equal(boolean(index), ScalarTerm::boolean(true));
    let alternatives = Proposition::Disjunction(vec![positive(1), positive(2)]);
    let packaged = Proposition::Conjunction(vec![
        Proposition::Truth,
        Proposition::Conjunction(vec![alternatives.clone(), Proposition::Truth]),
    ]);
    let goal = Proposition::Disjunction(vec![positive(4), positive(3)]);
    let aliases = vec![
        Proposition::Equal(boolean(3), boolean(1)),
        Proposition::Equal(boolean(4), boolean(2)),
    ];
    for semantic_origin in [false, true] {
        let mut assumptions = vec![Proposition::Truth];
        let mut axioms = aliases.clone();
        if semantic_origin {
            axioms.push(packaged.clone());
        } else {
            assumptions.push(packaged.clone());
        }
        let selected = connected_cases(&goal, &assumptions, &axioms);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].projection, vec![1, 0]);
        let projected = selected[0].proof();
        accept_certificate(&context, &alternatives, &assumptions, &axioms, &projected)
            .expect("each projection retains its original premise index");
        let proof = super::super::super::build(&context, &goal, &assumptions, &axioms)
            .expect("reordered actuals transport through each exact Boolean case");
        accept_certificate(&context, &goal, &assumptions, &axioms, &proof)
            .expect("kernel replays projected cases and actual-value equations");

        let mut corrupted = projected.clone();
        let ProofRule::ConjunctionElimination { conjunct, .. } = &mut corrupted.rule else {
            panic!("nested case retains its projection")
        };
        *conjunct = 1;
        assert!(
            accept_certificate(&context, &alternatives, &assumptions, &axioms, &corrupted).is_err()
        );
        let (mut missing_assumptions, mut missing_axioms) = (assumptions.clone(), axioms.clone());
        if semantic_origin {
            missing_axioms.pop();
        } else {
            missing_assumptions.pop();
        }
        assert!(
            accept_certificate(
                &context,
                &goal,
                &missing_assumptions,
                &missing_axioms,
                &proof
            )
            .is_err()
        );
        assert!(
            super::super::super::build(&context, &goal, &missing_assumptions, &missing_axioms)
                .is_none()
        );
        let (mut changed_assumptions, mut changed_axioms) = (assumptions.clone(), axioms.clone());
        if semantic_origin {
            *changed_axioms.last_mut().unwrap() = Proposition::Truth;
        } else {
            *changed_assumptions.last_mut().unwrap() = Proposition::Truth;
        }
        assert!(
            accept_certificate(
                &context,
                &goal,
                &changed_assumptions,
                &changed_axioms,
                &proof
            )
            .is_err()
        );
        let mut missing_alias = axioms.clone();
        missing_alias[0] = Proposition::Truth;
        assert!(accept_certificate(&context, &goal, &assumptions, &missing_alias, &proof).is_err());
    }
}
