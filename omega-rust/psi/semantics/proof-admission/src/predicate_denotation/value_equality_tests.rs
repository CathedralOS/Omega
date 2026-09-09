use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

use super::{
    CheckedPredicateDenotations, PredicateDenotationError, check_predicate_denotations,
    check_predicate_denotations_with_value_equalities,
};
use crate::{PrimitiveJudgment, ProofNode, ProofRule};

fn integer_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).unwrap()
}

fn value(identity: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(integer_type()),
    )
}

fn literal(integer: i128) -> ScalarTerm {
    ScalarTerm::integer(integer_type(), IntegerValue::Signed(integer)).unwrap()
}

fn sum(left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
    ScalarTerm::WrappingIntegerAdd {
        scalar_type: integer_type(),
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn fixture() -> (PropositionContext, Proposition, Vec<Proposition>) {
    let context = PropositionContext::from_value_types((1..=5).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            if identity == 5 {
                ScalarType::Boolean
            } else {
                ScalarType::Integer(integer_type())
            },
        )
    }))
    .unwrap();
    let condition = ScalarTerm::value(ValueId::new(5).unwrap(), ScalarType::Boolean);
    let goal = Proposition::Equal(
        ScalarTerm::Boolean(true),
        ScalarTerm::IntegerLessOrEqual {
            scalar_type: integer_type(),
            left: Box::new(value(2)),
            right: Box::new(sum(value(1), literal(1))),
        },
    );
    let axioms = vec![
        Proposition::Equal(value(3), literal(1)),
        Proposition::Equal(value(4), sum(value(1), value(3))),
        Proposition::Equal(
            condition.clone(),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type: integer_type(),
                left: Box::new(value(2)),
                right: Box::new(value(4)),
            },
        ),
        Proposition::LessOrEqual(value(2), value(4)),
        Proposition::Equal(condition, ScalarTerm::Boolean(true)),
    ];
    (context, goal, axioms)
}

fn citation(checked: &CheckedPredicateDenotations<'_>, index: usize) -> ProofNode {
    ProofNode {
        conclusion: checked.goal().clone(),
        rule: ProofRule::SemanticAxiom { index },
    }
}

#[test]
fn nested_value_equations_transport_the_original_guard_without_new_axioms() {
    let (context, goal, axioms) = fixture();
    let original = axioms.clone();
    let checked =
        check_predicate_denotations_with_value_equalities(&context, &goal, &[], &axioms).unwrap();
    assert_eq!(checked.semantic_axioms().len(), axioms.len());
    assert_eq!(
        checked.goal(),
        &Proposition::LessOrEqual(value(2), sum(value(1), literal(1)))
    );
    for index in [3, 4] {
        checked
            .check_certificate(&context, &citation(&checked, index))
            .unwrap();
    }
    assert_eq!(axioms, original);
    assert_eq!(checked.original_goal, &goal);
    assert_eq!(checked.original_semantic_axioms, &axioms);

    let unchanged = check_predicate_denotations(&context, &goal, &[], &axioms).unwrap();
    assert!(
        unchanged
            .check_certificate(&context, &citation(&unchanged, 3))
            .is_err()
    );
}

#[test]
fn missing_definitions_or_changed_literal_policy_and_order_cannot_prove_the_guard() {
    for mutation in ["constant", "operation", "literal", "policy", "order"] {
        let (context, goal, mut axioms) = fixture();
        match mutation {
            "constant" => axioms[0] = Proposition::Truth,
            "operation" => axioms[1] = Proposition::Truth,
            "literal" => axioms[0] = Proposition::Equal(value(3), literal(2)),
            "policy" => {
                axioms[1] = Proposition::Equal(
                    value(4),
                    ScalarTerm::ExactIntegerAdd {
                        scalar_type: integer_type(),
                        left: Box::new(value(1)),
                        right: Box::new(value(3)),
                    },
                );
            }
            "order" => axioms[1] = Proposition::Equal(value(4), sum(value(3), value(1))),
            _ => unreachable!(),
        }
        let checked =
            check_predicate_denotations_with_value_equalities(&context, &goal, &[], &axioms)
                .unwrap();
        for index in [3, 4] {
            assert!(
                checked
                    .check_certificate(&context, &citation(&checked, index))
                    .is_err(),
                "{mutation}"
            );
        }
    }
}

#[test]
fn requirements_and_conditional_equalities_never_supply_global_definitions() {
    for location in ["requirements", "conjunction", "disjunction", "implication"] {
        let (context, goal, mut axioms) = fixture();
        let definition = std::mem::replace(&mut axioms[0], Proposition::Truth);
        let mut requirements = Vec::new();
        match location {
            "requirements" => requirements.push(definition),
            "conjunction" => {
                axioms[0] = Proposition::Conjunction(vec![definition, Proposition::Truth])
            }
            "disjunction" => {
                axioms[0] = Proposition::Disjunction(vec![definition, Proposition::Truth])
            }
            "implication" => {
                axioms[0] = Proposition::Implication {
                    premise: Box::new(Proposition::Falsehood),
                    conclusion: Box::new(definition),
                }
            }
            _ => unreachable!(),
        }
        let checked = check_predicate_denotations_with_value_equalities(
            &context,
            &goal,
            &requirements,
            &axioms,
        )
        .unwrap();
        assert!(
            checked
                .check_certificate(&context, &citation(&checked, 3))
                .is_err(),
            "{location}"
        );
    }
    let (context, _, _) = fixture();
    let goal = Proposition::Equal(value(3), literal(1));
    let checked =
        check_predicate_denotations_with_value_equalities(&context, &goal, &[], &[]).unwrap();
    assert!(
        checked
            .check_certificate(
                &context,
                &ProofNode {
                    conclusion: checked.goal().clone(),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                }
            )
            .is_err(),
        "the goal cannot establish its own equality"
    );
}

#[test]
fn wrong_carrier_and_changed_replay_context_reject() {
    let (context, goal, mut axioms) = fixture();
    let wrong = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    axioms[0] = Proposition::Equal(
        value(3),
        ScalarTerm::integer(wrong, IntegerValue::Unsigned(1)).unwrap(),
    );
    assert!(matches!(
        check_predicate_denotations_with_value_equalities(&context, &goal, &[], &axioms),
        Err(PredicateDenotationError::Malformed(_))
    ));
    let (context, goal, axioms) = fixture();
    let checked =
        check_predicate_denotations_with_value_equalities(&context, &goal, &[], &axioms).unwrap();
    let changed = PropositionContext::from_value_types(
        (1..=5).map(|identity| (ValueId::new(identity).unwrap(), ScalarType::Boolean)),
    )
    .unwrap();
    assert!(matches!(
        checked.check_certificate(&changed, &citation(&checked, 3)),
        Err(PredicateDenotationError::Malformed(_))
    ));
}

#[test]
fn cyclic_definitions_reject_but_shared_acyclic_aliases_preserve_identity() {
    let (context, goal, mut axioms) = fixture();
    axioms[0] = Proposition::Equal(value(3), value(4));
    axioms[1] = Proposition::Equal(value(4), value(3));
    assert!(matches!(
        check_predicate_denotations_with_value_equalities(&context, &goal, &[], &axioms),
        Err(PredicateDenotationError::CyclicValueEquality)
    ));
    let goal = Proposition::Equal(value(4), sum(value(1), value(1)));
    let axioms = [
        Proposition::Equal(value(3), value(1)),
        Proposition::Equal(value(4), sum(value(3), value(3))),
    ];
    let checked =
        check_predicate_denotations_with_value_equalities(&context, &goal, &[], &axioms).unwrap();
    checked
        .check_certificate(
            &context,
            &ProofNode {
                conclusion: checked.goal().clone(),
                rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
            },
        )
        .unwrap();
}

#[test]
fn shared_budget_bounds_input_depth_and_repeated_dag_expansion() {
    let context = PropositionContext::from_value_types((1..=16).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer_type()),
        )
    }))
    .unwrap();
    let goal = Proposition::Equal(value(16), literal(1));
    let axioms = (2..=16)
        .map(|identity| {
            Proposition::Equal(
                value(identity),
                sum(value(identity - 1), value(identity - 1)),
            )
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        check_predicate_denotations_with_value_equalities(&context, &goal, &[], &axioms),
        Err(PredicateDenotationError::ResourceLimitExceeded)
    ));
    let inputs = vec![Proposition::Truth; 4096];
    assert!(matches!(
        check_predicate_denotations_with_value_equalities(
            &context,
            &Proposition::Truth,
            &[],
            &inputs
        ),
        Err(PredicateDenotationError::ResourceLimitExceeded)
    ));
    let mut deep = value(1);
    for _ in 0..64 {
        deep = sum(deep, literal(1));
    }
    assert!(matches!(
        check_predicate_denotations_with_value_equalities(
            &context,
            &Proposition::Equal(value(2), deep),
            &[],
            &[]
        ),
        Err(PredicateDenotationError::ResourceLimitExceeded)
    ));
}
