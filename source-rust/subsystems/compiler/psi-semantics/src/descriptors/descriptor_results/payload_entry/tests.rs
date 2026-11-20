use super::*;
use proof_admission::check_certificate;
use semantic_vocabulary::{PropositionContext, ScalarTerm, ScalarType, ValueId};

#[test]
fn disjunction_transport_checks_every_alternative_in_its_own_scope() {
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let values = [1, 2, 3].map(|index| ValueId::new(index).unwrap());
    let context = PropositionContext::from_value_types(
        values.map(|value| (value, ScalarType::Integer(integer))),
    )
    .unwrap();
    let finite = values.map(|value| {
        Proposition::LessThan(
            ScalarTerm::value(value, ScalarType::Integer(integer)),
            ScalarTerm::integer(integer, IntegerValue::Unsigned(10)).unwrap(),
        )
    });
    let mathematical = finite.each_ref().map(|proposition| {
        proof_admission::lift_fixed_integer_relation(proposition).unwrap()
    });
    let requirements = [Proposition::Conjunction(vec![
        Proposition::Truth,
        Proposition::Disjunction(finite[..2].to_vec()),
    ])];
    let goal = Proposition::Disjunction(mathematical[..2].to_vec());
    let proof = prove(&context, &goal, &requirements, 0).unwrap();
    check_certificate(&context, &goal, &requirements, &[], &proof).unwrap();
    proof_admission::verify_bounded_certificate(
        &context,
        &goal,
        &requirements,
        &[],
        &proof,
        &mut proof_admission::Budget::default(),
    )
    .unwrap();
    assert!(prove(&context, &mathematical[0], &requirements, 0).is_none());
    let wrong = Proposition::Disjunction(vec![
        mathematical[0].clone(),
        mathematical[2].clone(),
    ]);
    assert!(prove(&context, &wrong, &requirements, 0).is_none());
}

#[test]
fn disjunction_proof_uses_one_established_alternative_without_inventing_the_other() {
    let left = ValueId::new(1).unwrap();
    let right = ValueId::new(2).unwrap();
    let context = PropositionContext::from_value_types([
        (left, ScalarType::Boolean),
        (right, ScalarType::Boolean),
    ])
    .unwrap();
    let predicates = [left, right].map(|value| {
        Proposition::Equal(
            ScalarTerm::value(value, ScalarType::Boolean),
            ScalarTerm::Boolean(true),
        )
    });
    let goal = Proposition::Disjunction(predicates.to_vec());
    for predicate in &predicates {
        let requirements = [predicate.clone()];
        let checked =
            proof_admission::check_predicate_denotations_with_value_equalities(
                &context,
                &goal,
                &requirements,
                &[],
            )
            .unwrap();
        let proof = prove(&context, checked.goal(), checked.requirements(), 0).unwrap();
        assert!(checked.check_certificate(&context, &proof).is_ok());
    }
    assert!(prove(&context, &goal, &[], 0).is_none());
    assert!(
        prove(
            &context,
            &Proposition::Conjunction(predicates.to_vec()),
            &predicates[..1],
            0
        )
        .is_none()
    );
}

#[test]
fn selected_finite_relation_uses_the_kernel_mathematical_citation_bridge() {
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let value = ValueId::new(1).unwrap();
    let other = ValueId::new(2).unwrap();
    let context = PropositionContext::from_value_types([
        (value, ScalarType::Integer(integer)),
        (other, ScalarType::Integer(integer)),
    ])
    .unwrap();
    let finite = Proposition::LessThan(
        ScalarTerm::value(value, ScalarType::Integer(integer)),
        ScalarTerm::integer(integer, IntegerValue::Unsigned(10)).unwrap(),
    );
    let goal = proof_admission::lift_fixed_integer_relation(&finite).unwrap();
    let requirements = [Proposition::Conjunction(vec![
        Proposition::Truth,
        finite.clone(),
    ])];
    let proof = prove(&context, &goal, &requirements, 0).unwrap();
    assert!(check_certificate(&context, &goal, &requirements, &[], &proof).is_ok());
    assert!(prove(&context, &goal, &[], 0).is_none());
    let reverse_requirements = [goal];
    let reverse = prove(&context, &finite, &reverse_requirements, 0).unwrap();
    assert!(
        check_certificate(&context, &finite, &reverse_requirements, &[], &reverse)
            .is_ok()
    );
    let wrong = Proposition::LessThan(
        ScalarTerm::value(other, ScalarType::Integer(integer)),
        ScalarTerm::integer(integer, IntegerValue::Unsigned(10)).unwrap(),
    );
    assert!(prove(&context, &wrong, &reverse_requirements, 0).is_none());
}
