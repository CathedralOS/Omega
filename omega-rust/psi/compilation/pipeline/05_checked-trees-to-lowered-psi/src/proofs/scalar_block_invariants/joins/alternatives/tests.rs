use super::super::{ArrivalEquations, scoped_goal};
use semantic_vocabulary::{
    IntegerSign, IntegerType, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};
use std::collections::BTreeSet;

fn value(index: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(index).unwrap(),
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
    )
}

#[test]
fn alternative_exit_demands_preserve_each_result_binding() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let context = PropositionContext::from_value_types(
        (1..=5).map(|index| (ValueId::new(index).unwrap(), scalar_type)),
    )
    .unwrap();
    let scope = PropositionContext::from_value_types(
        (1..=3).map(|index| (ValueId::new(index).unwrap(), scalar_type)),
    )
    .unwrap();
    let allowed = (1..=3)
        .map(|index| ValueId::new(index).unwrap())
        .collect::<BTreeSet<_>>();
    let goal = Proposition::LessOrEqual(value(4), value(1));
    let alternatives = [Proposition::Disjunction(vec![
        Proposition::Equal(value(4), value(2)),
        Proposition::Conjunction(vec![
            Proposition::Equal(value(4), value(5)),
            Proposition::Equal(value(5), value(3)),
        ]),
    ])];
    let mut remaining = 1024;
    let unconditional = ArrivalEquations::new(&alternatives, &mut remaining).unwrap();
    assert!(
        scoped_goal(
            &goal,
            &unconditional,
            &allowed,
            &scope,
            &context,
            None,
            &mut remaining
        )
        .is_none()
    );
    let demands = ArrivalEquations::alternatives(&alternatives, &mut remaining)
        .iter()
        .map(|equations| {
            scoped_goal(
                &goal,
                equations,
                &allowed,
                &scope,
                &context,
                None,
                &mut remaining,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        demands,
        vec![
            Proposition::LessOrEqual(value(2), value(1)),
            Proposition::LessOrEqual(value(3), value(1))
        ]
    );
}

#[test]
fn alternative_exit_contexts_do_not_mix_arms_or_open_implications() {
    let left = Proposition::Equal(value(4), value(2));
    let right = Proposition::Equal(value(5), value(3));
    let opaque = Proposition::Implication {
        premise: Box::new(left.clone()),
        conclusion: Box::new(right.clone()),
    };
    let axioms = [
        opaque.clone(),
        Proposition::Disjunction(vec![left.clone(), right.clone()]),
    ];
    let mut remaining = 128;
    assert_eq!(
        super::contexts(&axioms, &mut remaining),
        vec![vec![&opaque, &left], vec![&opaque, &right]]
    );
    let mut remaining = 2;
    assert!(super::contexts(&axioms, &mut remaining).is_empty());
    assert_eq!(remaining, 0);
}
