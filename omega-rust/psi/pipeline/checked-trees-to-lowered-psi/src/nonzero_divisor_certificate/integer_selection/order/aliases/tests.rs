use super::*;
use proof_admission::check_certificate;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, PropositionContext, ScalarType, ValueId,
};

fn integer_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 16).unwrap()
}

fn value(identity: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(integer_type()),
    )
}

fn literal(number: u128) -> ScalarTerm {
    ScalarTerm::integer(integer_type(), IntegerValue::Unsigned(number)).unwrap()
}

fn context() -> PropositionContext {
    PropositionContext::from_value_types((1..=4).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer_type()),
        )
    }))
    .unwrap()
}

#[test]
fn closed_bounds_cross_each_cited_result_alias() {
    let semantic_axioms = [
        Proposition::Equal(value(1), value(2)),
        Proposition::Equal(value(2), value(3)),
        Proposition::Equal(value(3), literal(7)),
    ];
    for goal in [
        Proposition::LessOrEqual(value(1), literal(255)),
        Proposition::LessOrEqual(literal(0), value(1)),
    ] {
        let proof = prove(&goal, &[], &semantic_axioms).expect("literal result bound");
        check_certificate(&context(), &goal, &[], &semantic_axioms, &proof).unwrap();
        assert!(check_certificate(&context(), &goal, &[], &semantic_axioms[..2], &proof).is_err());
    }
}

#[test]
fn conjoined_bounds_keep_projection_and_alias_custody() {
    let assumptions = [Proposition::Conjunction(vec![
        Proposition::Truth,
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(literal(7), value(1)),
            Proposition::LessOrEqual(value(1), literal(200)),
        ]),
    ])];
    let semantic_axioms = [
        Proposition::Equal(value(3), value(2)),
        Proposition::Equal(value(2), value(1)),
    ];
    for goal in [
        Proposition::LessOrEqual(value(3), literal(255)),
        Proposition::LessOrEqual(literal(0), value(3)),
    ] {
        let proof = prove(&goal, &assumptions, &semantic_axioms).expect("projected result bound");
        check_certificate(&context(), &goal, &assumptions, &semantic_axioms, &proof).unwrap();
        assert!(
            check_certificate(
                &context(),
                &goal,
                &[Proposition::Truth],
                &semantic_axioms,
                &proof
            )
            .is_err()
        );
        assert!(check_certificate(&context(), &goal, &assumptions, &[], &proof).is_err());
    }
}

#[test]
fn both_symbolic_endpoints_need_their_own_equality() {
    let assumptions = [Proposition::LessOrEqual(value(1), value(2))];
    let semantic_axioms = [
        Proposition::Equal(value(3), value(1)),
        Proposition::Equal(value(4), value(2)),
    ];
    let goal = Proposition::LessOrEqual(value(3), value(4));
    let proof = prove(&goal, &assumptions, &semantic_axioms).expect("two endpoint substitutions");
    check_certificate(&context(), &goal, &assumptions, &semantic_axioms, &proof).unwrap();
    assert!(prove(&goal, &assumptions, &semantic_axioms[..1]).is_none());
}

#[test]
fn unrelated_literals_and_strict_facts_do_not_mint_order_rules() {
    let goal = Proposition::LessOrEqual(value(1), literal(255));
    for semantic_axioms in [
        vec![Proposition::Equal(value(2), literal(7))],
        vec![Proposition::Equal(value(1), literal(256))],
        vec![Proposition::LessThan(value(1), literal(256))],
    ] {
        assert!(
            prove(&goal, &[], &semantic_axioms).is_none(),
            "{semantic_axioms:?}"
        );
    }
}

#[test]
fn mathematical_goal_keeps_the_projected_scalar_conclusion() {
    let scalar_goal = Proposition::LessOrEqual(value(1), literal(255));
    let assumptions = [Proposition::Conjunction(vec![
        Proposition::LessOrEqual(literal(7), value(1)),
        scalar_goal.clone(),
    ])];
    let goal = proof_admission::lift_fixed_integer_relation(&scalar_goal).unwrap();
    let proof = super::super::super::build(&context(), &goal, &assumptions, &[])
        .expect("mathematical bound from exact scalar conjunction projection");
    check_certificate(&context(), &goal, &assumptions, &[], &proof).unwrap();
    let ProofRule::IntegerLessOrEqualSubstitution { relation, .. } = &proof.rule else {
        panic!("projection needs an explicit order normalization step");
    };
    assert_eq!(relation.conclusion, scalar_goal);
    assert!(matches!(
        relation.rule,
        ProofRule::ConjunctionElimination { .. }
    ));
}
