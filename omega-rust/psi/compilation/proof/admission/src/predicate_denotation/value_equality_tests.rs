use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

use super::{
    CheckedPredicateDenotations, PredicateDenotationError,
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

#[test]
fn mixed_integer_predicate_denotation_keeps_exact_value_carriers() {
    use semantic_vocabulary::IntegerMathTerm;
    let signed = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let left_id = ValueId::new(1).unwrap();
    let right_id = ValueId::new(2).unwrap();
    let context = PropositionContext::from_value_types([
        (left_id, ScalarType::Integer(signed)),
        (right_id, ScalarType::Integer(unsigned)),
    ])
    .unwrap();
    let left = ScalarTerm::value(left_id, ScalarType::Integer(signed));
    let right = ScalarTerm::value(right_id, ScalarType::Integer(unsigned));
    let comparison = ScalarTerm::integer_less_than(signed, left, right).unwrap();
    let goal = Proposition::Equal(comparison, ScalarTerm::Boolean(true));
    let denotation = super::check_predicate_denotations(&context, &goal, &[], &[]).unwrap();
    assert_eq!(
        denotation.goal(),
        &Proposition::IntegerMathLessThan(
            IntegerMathTerm::MathValue {
                source_type: signed,
                value: left_id
            },
            IntegerMathTerm::MathValue {
                source_type: unsigned,
                value: right_id
            },
        )
    );
    let proof = ProofNode {
        conclusion: denotation.goal().clone(),
        rule: ProofRule::Assumption { index: 0 },
    };
    let premises = [goal.clone()];
    let denotation = super::check_predicate_denotations(&context, &goal, &premises, &[]).unwrap();
    denotation.check_certificate(&context, &proof).unwrap();
}

#[test]
fn mixed_math_guard_substitutes_only_cited_endpoints_and_representable_literals() {
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, IntegerMathTerm, PlaceId, StructuralFieldId,
        StructuralPlaceKind,
    };
    let signed = integer_type();
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let context = PropositionContext::from_value_types_and_places(
        [
            (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(unsigned)),
        ],
        [(
            PlaceId::new(1).unwrap(),
            StructuralPlaceKind::Parameter {
                position: 0,
                is_self: true,
            },
        )],
    )
    .unwrap();
    let field = ScalarTerm::integer_field_path(
        PlaceId::new(1).unwrap(),
        vec![CanonicalStructuralPathSegment::Field(
            StructuralFieldId::new(1).unwrap(),
        )],
        signed,
    );
    let right = ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Integer(unsigned));
    let guard = Proposition::IntegerMathLessThan(
        IntegerMathTerm::MathValue {
            source_type: signed,
            value: ValueId::new(1).unwrap(),
        },
        IntegerMathTerm::MathValue {
            source_type: unsigned,
            value: ValueId::new(2).unwrap(),
        },
    );
    for bound in [12, u64::MAX as u128] {
        let equations = [
            Proposition::Equal(value(1), field.clone()),
            Proposition::Equal(
                right.clone(),
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(bound)).unwrap(),
            ),
        ];
        let denotation =
            super::check_value_equality_denotation(&context, &guard, &equations).unwrap();
        if bound == 12 {
            assert_eq!(
                denotation,
                Proposition::LessThan(field.clone(), literal(12))
            );
        } else {
            let Proposition::Equal(first, second) = denotation else {
                panic!("unrepresentable literal must retain independent carriers")
            };
            let comparison = if first == ScalarTerm::Boolean(true) {
                second
            } else {
                first
            };
            let ScalarTerm::IntegerLessThan { left, right, .. } = comparison else {
                panic!("mixed comparison retained")
            };
            assert_eq!(left.scalar_type(), ScalarType::Integer(signed));
            assert_eq!(right.scalar_type(), ScalarType::Integer(unsigned));
        }
        let missing =
            super::check_value_equality_denotation(&context, &guard, &equations[..1]).unwrap();
        assert_ne!(missing, Proposition::LessThan(field.clone(), literal(12)));
    }
}

#[test]
fn concatenated_live_length_sum_uses_only_cited_integer_equations() {
    use semantic_vocabulary::IntegerMathTerm;
    let (context, _, _) = fixture();
    let embedded = |identity| IntegerMathTerm::MathValue {
        source_type: integer_type(),
        value: ValueId::new(identity).unwrap(),
    };
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::Add(Box::new(embedded(1)), Box::new(embedded(2))),
        IntegerMathTerm::literal(IntegerValue::Signed(8)),
    );
    let equations = vec![
        Proposition::Equal(value(1), literal(3)),
        Proposition::Equal(value(2), literal(4)),
    ];
    let expected = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::Add(
            Box::new(IntegerMathTerm::literal(IntegerValue::Signed(3))),
            Box::new(IntegerMathTerm::literal(IntegerValue::Signed(4))),
        ),
        IntegerMathTerm::literal(IntegerValue::Signed(8)),
    );
    assert_eq!(
        super::check_value_equality_denotation(&context, &goal, &equations).unwrap(),
        expected
    );
    assert_ne!(
        super::check_value_equality_denotation(&context, &goal, &equations[..1]).unwrap(),
        expected
    );
    let nested = vec![
        Proposition::Equal(
            value(1),
            ScalarTerm::exact_integer_add(integer_type(), value(3), literal(1)).unwrap(),
        ),
        Proposition::Equal(value(3), literal(2)),
        equations[1].clone(),
    ];
    let transported = super::check_value_equality_denotation(&context, &goal, &nested).unwrap();
    let expected_nested = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::Add(
            Box::new(IntegerMathTerm::Add(
                Box::new(IntegerMathTerm::literal(IntegerValue::Signed(2))),
                Box::new(IntegerMathTerm::literal(IntegerValue::Signed(1))),
            )),
            Box::new(IntegerMathTerm::literal(IntegerValue::Signed(4))),
        ),
        IntegerMathTerm::literal(IntegerValue::Signed(8)),
    );
    assert_eq!(transported, expected_nested);
    crate::decide_primitive(
        &context,
        &transported,
        PrimitiveJudgment::ClosedIntegerRelation,
    )
    .unwrap();
    let wrapped = vec![
        Proposition::Equal(value(1), sum(value(3), literal(1))),
        equations[1].clone(),
    ];
    let transported = super::check_value_equality_denotation(&context, &goal, &wrapped).unwrap();
    let Proposition::IntegerMathLessOrEqual(IntegerMathTerm::Add(left, _), _) = transported else {
        panic!("exact mathematical sum")
    };
    assert_eq!(*left, embedded(1));
}

#[test]
fn mathematical_leaf_order_expands_only_cited_exact_scalar_definitions() {
    use semantic_vocabulary::IntegerMathTerm;
    let (context, _, _) = fixture();
    let embedded = |identity| IntegerMathTerm::MathValue {
        source_type: integer_type(),
        value: ValueId::new(identity).unwrap(),
    };
    let original = Proposition::IntegerMathLessOrEqual(
        embedded(1),
        IntegerMathTerm::literal(IntegerValue::Signed(100)),
    );
    let expected = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::Add(Box::new(embedded(2)), Box::new(embedded(3))),
        IntegerMathTerm::literal(IntegerValue::Signed(100)),
    );
    let exact = Proposition::Equal(
        value(1),
        ScalarTerm::exact_integer_add(integer_type(), value(2), value(3)).unwrap(),
    );
    assert_eq!(
        super::check_value_equality_denotation(&context, &original, [&exact]).unwrap(),
        expected,
    );
    let transport = |equation: &Proposition| ProofNode {
        conclusion: expected.clone(),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(ProofNode {
                conclusion: original.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            equalities: vec![ProofNode {
                conclusion: equation.clone(),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }],
        },
    };
    crate::accept_certificate(
        &context,
        &expected,
        &[original.clone()],
        &[exact.clone()],
        &transport(&exact),
    )
    .expect("exact scalar definition and mathematical order share checked denotation");

    for changed in [
        Proposition::Equal(
            value(1),
            ScalarTerm::exact_integer_add(integer_type(), value(2), value(4)).unwrap(),
        ),
        Proposition::Equal(
            value(1),
            ScalarTerm::wrapping_integer_add(integer_type(), value(2), value(3)).unwrap(),
        ),
        Proposition::Equal(
            value(1),
            ScalarTerm::saturating_integer_add(integer_type(), value(2), value(3)).unwrap(),
        ),
    ] {
        assert_ne!(
            super::check_value_equality_denotation(&context, &original, [&changed]).unwrap(),
            expected,
        );
        // Rebind the citation itself to the current equation. Rejection must
        // come from the attempted semantic transport, not stale proof bytes.
        assert!(
            crate::accept_certificate(
                &context,
                &expected,
                &[original.clone()],
                &[changed.clone()],
                &transport(&changed),
            )
            .is_err()
        );
    }
    assert!(
        crate::accept_certificate(
            &context,
            &expected,
            &[original.clone()],
            &[],
            &transport(&exact),
        )
        .is_err()
    );
}
