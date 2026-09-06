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
fn negated_predicates_keep_exact_entry_and_site_premise_authority() {
    let context = boolean_context();
    let goal = observe(ScalarTerm::boolean_not(boolean(1)).unwrap(), true);
    let premise = observe(boolean(1), false);
    for semantic in [false, true] {
        let premises = [premise.clone()];
        let (requirements, axioms) = if semantic {
            (&[][..], &premises[..])
        } else {
            (&premises[..], &[][..])
        };
        let checked = check_predicate_denotations(&context, &goal, requirements, axioms).unwrap();
        let proof = citation(checked.goal(), semantic);
        checked.check_certificate(&context, &proof).unwrap();

        let absent = check_predicate_denotations(&context, &goal, &[], &[]).unwrap();
        assert!(absent.check_certificate(&context, &proof).is_err());
        for wrong in [observe(boolean(1), true), observe(boolean(2), false)] {
            let wrong = [wrong];
            let (requirements, axioms) = if semantic {
                (&[][..], &wrong[..])
            } else {
                (&wrong[..], &[][..])
            };
            let changed =
                check_predicate_denotations(&context, &goal, requirements, axioms).unwrap();
            assert!(changed.check_certificate(&context, &proof).is_err());
        }
    }
}

#[test]
fn closed_boolean_equality_and_nested_negation_preserve_both_polarities() {
    for left in [false, true] {
        for right in [false, true] {
            for positive in [false, true] {
                for reverse in [false, true] {
                    let equality = ScalarTerm::boolean_equal(
                        ScalarTerm::Boolean(left),
                        ScalarTerm::Boolean(right),
                    )
                    .unwrap();
                    let negated = ScalarTerm::boolean_not(equality).unwrap();
                    let goal = if reverse {
                        Proposition::Equal(ScalarTerm::Boolean(positive), negated)
                    } else {
                        observe(negated, positive)
                    };
                    let checked = check_predicate_denotations(
                        &PropositionContext::default(),
                        &goal,
                        &[],
                        &[],
                    )
                    .unwrap();
                    let expected = (left != right) == positive;
                    assert_eq!(
                        checked.goal(),
                        if expected {
                            &Proposition::Truth
                        } else {
                            &Proposition::Falsehood
                        }
                    );
                    let proof = ProofNode {
                        conclusion: checked.goal().clone(),
                        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                    };
                    assert_eq!(
                        checked
                            .check_certificate(&PropositionContext::default(), &proof)
                            .is_ok(),
                        expected
                    );
                }
            }
        }
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
fn integer_comparison_denotations_preserve_width_order_and_complement() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let integer_type = IntegerType::new(sign, 64).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let left = ScalarTerm::value(ValueId::new(1).unwrap(), scalar_type);
        let zero = ScalarTerm::integer(
            integer_type,
            match sign {
                IntegerSign::Signed => IntegerValue::Signed(0),
                IntegerSign::Unsigned => IntegerValue::Unsigned(0),
            },
        )
        .unwrap();
        let context =
            PropositionContext::from_value_types([(ValueId::new(1).unwrap(), scalar_type)])
                .unwrap();
        for inclusive in [false, true] {
            for positive in [false, true] {
                let term = if inclusive {
                    ScalarTerm::IntegerLessOrEqual {
                        scalar_type: integer_type,
                        left: Box::new(left.clone()),
                        right: Box::new(zero.clone()),
                    }
                } else {
                    ScalarTerm::IntegerLessThan {
                        scalar_type: integer_type,
                        left: Box::new(left.clone()),
                        right: Box::new(zero.clone()),
                    }
                };
                let goal = observe(term, positive);
                let expected = match (inclusive, positive) {
                    (false, true) => Proposition::LessThan(left.clone(), zero.clone()),
                    (false, false) => Proposition::LessOrEqual(zero.clone(), left.clone()),
                    (true, true) => Proposition::LessOrEqual(left.clone(), zero.clone()),
                    (true, false) => Proposition::LessThan(zero.clone(), left.clone()),
                };
                let axioms = [expected.clone()];
                let checked = check_predicate_denotations(&context, &goal, &[], &axioms).unwrap();
                assert_eq!(checked.goal(), &expected);
                checked
                    .check_certificate(&context, &citation(checked.goal(), true))
                    .unwrap();
                let wrong = [Proposition::LessThan(zero.clone(), zero.clone())];
                let changed = check_predicate_denotations(&context, &goal, &[], &wrong).unwrap();
                assert!(
                    changed
                        .check_certificate(&context, &citation(checked.goal(), true))
                        .is_err()
                );
            }
        }
    }
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
fn integer_equality_and_order_complements_preserve_signed_and_unsigned_boundaries() {
    for width in [8, 64] {
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            let integer_type = IntegerType::new(sign, width).unwrap();
            let scalar_type = ScalarType::Integer(integer_type);
            let context = PropositionContext::from_value_types(
                [1, 2].map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
            )
            .unwrap();
            let values = match sign {
                IntegerSign::Signed => vec![
                    TestValue::Signed(-(1_i128 << (width - 1))),
                    TestValue::Signed(-1),
                    TestValue::Signed(0),
                    TestValue::Signed(1),
                    TestValue::Signed((1_i128 << (width - 1)) - 1),
                ],
                IntegerSign::Unsigned => vec![
                    TestValue::Unsigned(0),
                    TestValue::Unsigned(1),
                    TestValue::Unsigned((1_u128 << width) - 1),
                ],
            };
            let left = ScalarTerm::value(ValueId::new(1).unwrap(), scalar_type);
            let right = ScalarTerm::value(ValueId::new(2).unwrap(), scalar_type);
            for relation in 0..3 {
                let term = match relation {
                    0 => ScalarTerm::IntegerEqual {
                        scalar_type: integer_type,
                        left: Box::new(left.clone()),
                        right: Box::new(right.clone()),
                    },
                    1 => ScalarTerm::IntegerLessThan {
                        scalar_type: integer_type,
                        left: Box::new(left.clone()),
                        right: Box::new(right.clone()),
                    },
                    _ => ScalarTerm::IntegerLessOrEqual {
                        scalar_type: integer_type,
                        left: Box::new(left.clone()),
                        right: Box::new(right.clone()),
                    },
                };
                for negated in [false, true] {
                    let term = if negated {
                        ScalarTerm::boolean_not(term.clone()).unwrap()
                    } else {
                        term.clone()
                    };
                    for positive in [false, true] {
                        let goal = observe(term.clone(), positive);
                        let checked =
                            check_predicate_denotations(&context, &goal, &[], &[]).unwrap();
                        for first in &values {
                            for second in &values {
                                let assignment = [*first, *second];
                                assert_eq!(
                                    evaluate_proposition(&goal, &assignment),
                                    evaluate_proposition(checked.goal(), &assignment),
                                    "{goal:?} under {assignment:?}"
                                );
                            }
                        }
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
        [(
            root,
            StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        )],
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
