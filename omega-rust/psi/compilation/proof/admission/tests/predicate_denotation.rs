use proof_admission::{
    PredicateDenotationError, PrimitiveJudgment, ProofNode, ProofRule, check_predicate_denotations,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IeeeFloatComparisonKind, IeeeFloatFormat,
    IeeeFloatStructuralField, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, PlaceId,
    Proposition, PropositionContext, ScalarTerm, ScalarType, StructuralFieldId,
    StructuralPlaceKind, ValueId,
};

fn boolean(identity: u64) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(identity).unwrap(), ScalarType::Boolean)
}

#[test]
fn data_identity_is_reflexive_but_distinct_subjects_need_a_premise() {
    use semantic_vocabulary::{StructuralIdentitySubject, StructuralTypeId};
    let left_root = PlaceId::new(1).unwrap();
    let right_root = PlaceId::new(2).unwrap();
    let context = PropositionContext::from_value_types_and_places(
        [],
        [
            (left_root, StructuralPlaceKind::Parameter { position: 0 }),
            (right_root, StructuralPlaceKind::Parameter { position: 1 }),
        ],
    )
    .unwrap();
    let carrier = StructuralTypeId::new(1).unwrap();
    let left = StructuralIdentitySubject::new(left_root, Vec::new(), carrier);
    let right = StructuralIdentitySubject::new(right_root, Vec::new(), carrier);
    let reflexive = Proposition::StructuralIdentity {
        left: left.clone(),
        right: left.clone(),
    };
    let checked = check_predicate_denotations(&context, &reflexive, &[], &[]).unwrap();
    assert_eq!(checked.goal(), &Proposition::Truth);
    checked
        .check_certificate(
            &context,
            &ProofNode {
                conclusion: Proposition::Truth,
                rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
            },
        )
        .unwrap();

    let distinct = Proposition::StructuralIdentity { left, right };
    let checked = check_predicate_denotations(&context, &distinct, &[], &[]).unwrap();
    assert_eq!(checked.goal(), &distinct);
    assert!(
        checked
            .check_certificate(&context, &citation(&distinct, false))
            .is_err()
    );
    let premises = [distinct.clone()];
    let checked = check_predicate_denotations(&context, &distinct, &premises, &[]).unwrap();
    checked
        .check_certificate(&context, &citation(&distinct, false))
        .unwrap();
}

#[test]
fn closed_ieee_order_preserves_nan_signed_zero_and_format() {
    use semantic_vocabulary::{IeeeFloatComparisonKind as Kind, IeeeFloatValue};
    for (left_binary32, right_binary32, left_binary64, right_binary64, equal, less, less_equal) in [
        (0x8000_0000, 0, 0x8000_0000_0000_0000, 0, true, false, true),
        (
            0x7fc0_0001,
            0,
            0x7ff8_0000_0000_0001,
            0,
            false,
            false,
            false,
        ),
        (
            0xff80_0000,
            0x7f80_0000,
            0xfff0_0000_0000_0000,
            0x7ff0_0000_0000_0000,
            false,
            true,
            true,
        ),
        (0xbf80_0000, 0, 0xbff0_0000_0000_0000, 0, false, true, true),
        (
            0x0000_0001,
            0,
            0x0000_0000_0000_0001,
            0,
            false,
            false,
            false,
        ),
    ] {
        for (left, right) in [
            (
                IeeeFloatValue::Binary32(left_binary32),
                IeeeFloatValue::Binary32(right_binary32),
            ),
            (
                IeeeFloatValue::Binary64(left_binary64),
                IeeeFloatValue::Binary64(right_binary64),
            ),
        ] {
            let format = left.format();
            for (kind, holds) in [
                (Kind::Equal, equal),
                (Kind::NotEqual, !equal),
                (Kind::LessThan, less),
                (Kind::LessOrEqual, less_equal),
                (Kind::NotLessThan, !less),
                (Kind::NotLessOrEqual, !less_equal),
            ] {
                let mut left = ScalarTerm::IeeeFloatLiteral(left);
                let mut right = ScalarTerm::IeeeFloatLiteral(right);
                if kind.is_symmetric() && left > right {
                    std::mem::swap(&mut left, &mut right);
                }
                let goal = Proposition::ScalarIeeeFloatComparison {
                    kind,
                    format,
                    left,
                    right,
                };
                let converted =
                    check_predicate_denotations(&PropositionContext::default(), &goal, &[], &[])
                        .unwrap();
                assert_eq!(
                    converted.goal(),
                    if holds {
                        &Proposition::Truth
                    } else {
                        &Proposition::Falsehood
                    },
                    "{format:?} {kind:?}: {goal:?}"
                );
            }
        }
    }
    let subject = ScalarTerm::value(
        ValueId::new(1).unwrap(),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
    );
    let goal = Proposition::ScalarIeeeFloatComparison {
        kind: Kind::Equal,
        format: IeeeFloatFormat::Binary64,
        left: subject.clone(),
        right: subject,
    };
    let context = PropositionContext::from_value_types([(
        ValueId::new(1).unwrap(),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
    )])
    .unwrap();
    assert_eq!(
        check_predicate_denotations(&context, &goal, &[], &[])
            .unwrap()
            .goal(),
        &goal,
        "unknown IEEE values are not reflexive"
    );
}

fn boolean_context() -> PropositionContext {
    PropositionContext::from_value_types(
        [1, 2].map(|identity| (ValueId::new(identity).unwrap(), ScalarType::Boolean)),
    )
    .unwrap()
}

fn observe(term: ScalarTerm, positive: bool) -> Proposition {
    Proposition::Equal(term, ScalarTerm::Boolean(positive))
}

fn citation(goal: &Proposition, semantic: bool) -> ProofNode {
    ProofNode {
        conclusion: goal.clone(),
        rule: if semantic {
            ProofRule::SemanticAxiom { index: 0 }
        } else {
            ProofRule::Assumption { index: 0 }
        },
    }
}

#[test]
fn nonliteral_boolean_equality_never_asserts_either_operand() {
    let context = boolean_context();
    let term = ScalarTerm::boolean_equal(boolean(1), boolean(2)).unwrap();
    let goal = observe(term.clone(), true);
    let requirements = [Proposition::Equal(boolean(1), boolean(2))];
    let checked = check_predicate_denotations(&context, &goal, &requirements, &[]).unwrap();
    checked
        .check_certificate(&context, &citation(checked.goal(), false))
        .unwrap();

    let truth_goal = observe(boolean(1), true);
    let equal_only = [goal];
    let checked = check_predicate_denotations(&context, &truth_goal, &equal_only, &[]).unwrap();
    assert!(
        checked
            .check_certificate(&context, &citation(checked.goal(), false))
            .is_err()
    );
    let unequal = observe(term, false);
    let checked = check_predicate_denotations(&context, &unequal, &requirements, &[]).unwrap();
    assert!(matches!(checked.goal(), Proposition::Disjunction(_)));
    assert!(
        checked
            .check_certificate(&context, &citation(checked.goal(), false))
            .is_err()
    );
}

#[test]
fn normalization_does_not_hide_malformed_operand_types_or_context_changes() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let wrong_type = ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Integer(integer_type));
    let malformed = observe(
        ScalarTerm::BooleanNot {
            operand: Box::new(wrong_type),
        },
        true,
    );
    assert!(matches!(
        check_predicate_denotations(&boolean_context(), &malformed, &[], &[]),
        Err(PredicateDenotationError::Malformed(_))
    ));

    let reflexive = Proposition::Equal(boolean(1), boolean(1));
    let checked = check_predicate_denotations(&boolean_context(), &reflexive, &[], &[]).unwrap();
    let proof = ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    };
    checked
        .check_certificate(&boolean_context(), &proof)
        .unwrap();
    let changed = PropositionContext::from_value_types([(
        ValueId::new(1).unwrap(),
        ScalarType::Integer(integer_type),
    )])
    .unwrap();
    assert!(checked.check_certificate(&changed, &proof).is_err());
    assert!(
        checked
            .check_certificate(&PropositionContext::default(), &proof)
            .is_err()
    );
}

#[test]
fn conjunction_conversion_preserves_falsehood_and_exact_premise_order() {
    let context = boolean_context();
    let goal =
        Proposition::Conjunction(vec![observe(boolean(1), false), observe(boolean(2), true)]);
    let requirements = [Proposition::Conjunction(vec![
        observe(ScalarTerm::boolean_not(boolean(1)).unwrap(), true),
        observe(boolean(2), true),
    ])];
    let checked = check_predicate_denotations(&context, &goal, &requirements, &[]).unwrap();
    checked
        .check_certificate(&context, &citation(checked.goal(), false))
        .unwrap();

    let false_goal = Proposition::Conjunction(vec![goal.clone(), Proposition::Falsehood]);
    let checked = check_predicate_denotations(&context, &false_goal, &requirements, &[]).unwrap();
    assert_eq!(checked.goal(), &Proposition::Falsehood);
    assert!(
        checked
            .check_certificate(&context, &citation(checked.goal(), false))
            .is_err()
    );
}

#[test]
fn conversion_bounds_input_depth_total_premises_and_boolean_expansion() {
    let context = boolean_context();
    let mut deep = boolean(1);
    for _ in 0..64 {
        deep = ScalarTerm::boolean_not(deep).unwrap();
    }
    assert!(matches!(
        check_predicate_denotations(&context, &observe(deep, true), &[], &[]),
        Err(PredicateDenotationError::ResourceLimitExceeded)
    ));

    let requirements = vec![Proposition::Truth; 2500];
    let axioms = vec![Proposition::Truth; 2500];
    assert!(matches!(
        check_predicate_denotations(&context, &Proposition::Truth, &requirements, &axioms),
        Err(PredicateDenotationError::ResourceLimitExceeded)
    ));

    let mut expanding = boolean(1);
    for _ in 0..14 {
        expanding = ScalarTerm::boolean_equal(expanding, boolean(2)).unwrap();
    }
    assert!(matches!(
        check_predicate_denotations(&context, &observe(expanding, true), &[], &[]),
        Err(PredicateDenotationError::ResourceLimitExceeded)
    ));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestValue {
    Boolean(bool),
    Signed(i128),
    Unsigned(u128),
}

fn evaluate_term(term: &ScalarTerm, values: &[TestValue; 2]) -> TestValue {
    let boolean_value = |value| match value {
        TestValue::Boolean(value) => value,
        _ => panic!("non-Boolean test operand"),
    };
    match term {
        ScalarTerm::Value { id, .. } => {
            if *id == ValueId::new(1).unwrap() {
                values[0]
            } else if *id == ValueId::new(2).unwrap() {
                values[1]
            } else {
                panic!("unknown test value")
            }
        }
        ScalarTerm::Boolean(value) => TestValue::Boolean(*value),
        ScalarTerm::Integer {
            value: IntegerValue::Signed(value),
            ..
        } => TestValue::Signed(*value),
        ScalarTerm::Integer {
            value: IntegerValue::Unsigned(value),
            ..
        } => TestValue::Unsigned(*value),
        ScalarTerm::BooleanNot { operand } => {
            TestValue::Boolean(!boolean_value(evaluate_term(operand, values)))
        }
        ScalarTerm::BooleanEqual { left, right } | ScalarTerm::IntegerEqual { left, right, .. } => {
            TestValue::Boolean(evaluate_term(left, values) == evaluate_term(right, values))
        }
        ScalarTerm::IntegerLessThan { left, right, .. } => TestValue::Boolean(
            compare_test_values(evaluate_term(left, values), evaluate_term(right, values)).is_lt(),
        ),
        ScalarTerm::IntegerLessOrEqual { left, right, .. } => TestValue::Boolean(
            !compare_test_values(evaluate_term(left, values), evaluate_term(right, values)).is_gt(),
        ),
        _ => panic!("test evaluator does not interpret unrelated terms"),
    }
}

fn compare_test_values(left: TestValue, right: TestValue) -> std::cmp::Ordering {
    match (left, right) {
        (TestValue::Signed(left), TestValue::Signed(right)) => left.cmp(&right),
        (TestValue::Unsigned(left), TestValue::Unsigned(right)) => left.cmp(&right),
        _ => panic!("test comparison requires same integer carriers"),
    }
}

fn evaluate_proposition(proposition: &Proposition, values: &[TestValue; 2]) -> bool {
    match proposition {
        Proposition::Truth => true,
        Proposition::Falsehood => false,
        Proposition::Equal(left, right) => {
            evaluate_term(left, values) == evaluate_term(right, values)
        }
        Proposition::LessThan(left, right) => {
            compare_test_values(evaluate_term(left, values), evaluate_term(right, values)).is_lt()
        }
        Proposition::LessOrEqual(left, right) => {
            !compare_test_values(evaluate_term(left, values), evaluate_term(right, values)).is_gt()
        }
        Proposition::Conjunction(children) => children
            .iter()
            .all(|child| evaluate_proposition(child, values)),
        Proposition::Disjunction(children) => children
            .iter()
            .any(|child| evaluate_proposition(child, values)),
        Proposition::Implication {
            premise,
            conclusion,
        } => !evaluate_proposition(premise, values) || evaluate_proposition(conclusion, values),
        _ => panic!("test evaluator must not assign meanings to opaque atoms"),
    }
}

#[test]
fn all_small_boolean_expressions_preserve_truth_under_every_assignment() {
    let atoms = vec![
        ScalarTerm::Boolean(false),
        ScalarTerm::Boolean(true),
        boolean(1),
        boolean(2),
    ];
    let extend = |expressions: &[ScalarTerm]| {
        let mut output = expressions.to_vec();
        for left in expressions {
            output.push(ScalarTerm::boolean_not(left.clone()).unwrap());
            for right in expressions {
                output.push(ScalarTerm::boolean_equal(left.clone(), right.clone()).unwrap());
            }
        }
        output
    };
    // All trees with at most two BoolNot/BooleanEqual levels: 624 syntactic
    // expressions, both observed polarities, both equality orientations and
    // all four assignments. The evaluator above does not call conversion.
    let expressions = extend(&extend(&atoms));
    assert_eq!(expressions.len(), 624);
    let context = boolean_context();
    for expression in expressions {
        for positive in [false, true] {
            for reversed in [false, true] {
                let goal = if reversed {
                    Proposition::Equal(ScalarTerm::Boolean(positive), expression.clone())
                } else {
                    observe(expression.clone(), positive)
                };
                let checked = check_predicate_denotations(&context, &goal, &[], &[]).unwrap();
                for first in [false, true] {
                    for second in [false, true] {
                        let values = [TestValue::Boolean(first), TestValue::Boolean(second)];
                        assert_eq!(
                            evaluate_proposition(&goal, &values),
                            evaluate_proposition(checked.goal(), &values),
                            "{goal:?} under {values:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn ieee_and_mathematical_integer_atoms_are_preserved_without_new_laws() {
    let root = PlaceId::new(1).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    let context = PropositionContext::from_value_types_and_places(
        [(ValueId::new(1).unwrap(), ScalarType::Integer(integer_type))],
        [(root, StructuralPlaceKind::Parameter { position: 0 })],
    )
    .unwrap();
    let field = IeeeFloatStructuralField::new(
        root,
        vec![CanonicalStructuralPathSegment::Field(
            StructuralFieldId::new(1).unwrap(),
        )],
    )
    .unwrap();
    let subject = IntegerMathTerm::MathValue {
        source_type: integer_type,
        value: ValueId::new(1).unwrap(),
    };
    let mut terms = [
        subject.clone(),
        IntegerMathTerm::literal(IntegerValue::Signed(0)),
    ];
    terms.sort();
    let mut atoms = vec![
        Proposition::IntegerMathEqual(terms[0].clone(), terms[1].clone()),
        Proposition::IntegerMathLessThan(subject.clone(), terms[0].clone()),
        Proposition::IntegerMathLessOrEqual(subject, terms[1].clone()),
    ];
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        for kind in [
            IeeeFloatComparisonKind::Equal,
            IeeeFloatComparisonKind::NotEqual,
        ] {
            // Even comparing the exact same IEEE leaf must not become Truth:
            // that would silently assume away NaNs.
            atoms.push(Proposition::IeeeFloatComparison {
                kind,
                format,
                left: field.clone(),
                right: field.clone(),
            });
        }
    }
    for goal in atoms {
        let checked = check_predicate_denotations(&context, &goal, &[], &[]).unwrap();
        assert_eq!(checked.goal(), &goal);
        let proof = ProofNode {
            conclusion: checked.goal().clone(),
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        };
        assert!(checked.check_certificate(&context, &proof).is_err());
        let requirements = [goal.clone()];
        let checked = check_predicate_denotations(&context, &goal, &requirements, &[]).unwrap();
        assert_eq!(checked.requirements(), &requirements);
        checked
            .check_certificate(&context, &citation(checked.goal(), false))
            .unwrap();
    }
}
