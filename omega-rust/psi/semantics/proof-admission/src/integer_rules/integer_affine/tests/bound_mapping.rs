use super::{literal, value};
use crate::integer_rules::integer_affine::witness_checking::CheckedIntegerEndpointStep;
use crate::integer_rules::integer_affine::{
    CheckedIntegerAffineForm, IntegerAffineBoundConversionError, IntegerAffineWitness,
    IntegerAffineWitnessError, check_integer_affine_bound_conversion, check_integer_affine_witness,
    integer_affine_truth_bounds, map_integer_affine_bound,
};
use semantic_vocabulary::IntegerMathTerm;
use semantic_vocabulary::IntegerType;
use semantic_vocabulary::IntegerValue;
use semantic_vocabulary::Proposition;
use semantic_vocabulary::PropositionContext;
use semantic_vocabulary::ScalarTerm;
use semantic_vocabulary::ScalarType;
use semantic_vocabulary::{IntegerSign, ValueId};

#[test]
fn maps_upper_and_lower_bounds_across_every_coefficient_sign() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let root = value(1, integer_type);
    let target = value(2, integer_type);
    let literal = |value| literal(integer_type, value);
    let form = |coefficient, offset| CheckedIntegerAffineForm {
        root: root.clone(),
        target: target.clone(),
        integer_type,
        coefficient,
        offset,
        endpoint_steps: vec![
            CheckedIntegerEndpointStep::Multiply(coefficient),
            CheckedIntegerEndpointStep::Add(offset),
        ],
    };
    let upper = Proposition::LessOrEqual(root.clone(), literal(4));
    let lower = Proposition::LessOrEqual(literal(-3), root.clone());

    assert_eq!(
        check_integer_affine_bound_conversion(
            &form(2, 1),
            &upper,
            &Proposition::LessOrEqual(target.clone(), literal(9)),
        ),
        Ok(()),
    );
    assert_eq!(
        check_integer_affine_bound_conversion(
            &form(2, 1),
            &lower,
            &Proposition::LessOrEqual(literal(-5), target.clone()),
        ),
        Ok(()),
    );
    assert_eq!(
        check_integer_affine_bound_conversion(
            &form(-2, 1),
            &upper,
            &Proposition::LessOrEqual(literal(-7), target.clone()),
        ),
        Ok(()),
    );
    assert_eq!(
        check_integer_affine_bound_conversion(
            &form(-2, 1),
            &lower,
            &Proposition::LessOrEqual(target.clone(), literal(7)),
        ),
        Ok(()),
    );
    assert_eq!(
        check_integer_affine_bound_conversion(
            &form(0, 5),
            &upper,
            &Proposition::LessOrEqual(target.clone(), literal(5)),
        ),
        Ok(()),
    );
    assert_eq!(
        check_integer_affine_bound_conversion(
            &form(0, 5),
            &lower,
            &Proposition::LessOrEqual(literal(5), target.clone()),
        ),
        Ok(()),
    );
    assert_eq!(
        integer_affine_truth_bounds(&form(0, 5)),
        Ok(vec![
            Proposition::LessOrEqual(literal(5), target.clone()),
            Proposition::LessOrEqual(target, literal(5)),
        ]),
    );
}

#[test]
fn affine_bound_mapping_rejects_shape_direction_and_arithmetic_drift() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let root = value(1, integer_type);
    let target = value(2, integer_type);
    let literal = |value| literal(integer_type, value);
    let form = CheckedIntegerAffineForm {
        root: root.clone(),
        target: target.clone(),
        integer_type,
        coefficient: 2,
        offset: 1,
        endpoint_steps: vec![
            CheckedIntegerEndpointStep::Multiply(2),
            CheckedIntegerEndpointStep::Add(1),
        ],
    };
    assert_eq!(
        check_integer_affine_bound_conversion(
            &form,
            &Proposition::Equal(root.clone(), literal(4)),
            &Proposition::LessOrEqual(target.clone(), literal(9)),
        ),
        Err(IntegerAffineBoundConversionError::RootBoundNotLessOrEqual),
    );
    assert_eq!(
        check_integer_affine_bound_conversion(
            &form,
            &Proposition::LessOrEqual(value(3, integer_type), literal(4)),
            &Proposition::LessOrEqual(target.clone(), literal(9)),
        ),
        Err(IntegerAffineBoundConversionError::RootBoundMismatch),
    );
    assert_eq!(
        check_integer_affine_bound_conversion(
            &form,
            &Proposition::LessOrEqual(root.clone(), value(3, integer_type)),
            &Proposition::LessOrEqual(target.clone(), literal(9)),
        ),
        Err(IntegerAffineBoundConversionError::RootBoundNotTypedLiteral),
    );
    assert_eq!(
        check_integer_affine_bound_conversion(
            &form,
            &Proposition::LessOrEqual(root.clone(), literal(4)),
            &Proposition::LessOrEqual(literal(9), target.clone()),
        ),
        Err(IntegerAffineBoundConversionError::ConclusionMismatch),
    );
    assert_eq!(
        check_integer_affine_bound_conversion(
            &CheckedIntegerAffineForm {
                endpoint_steps: vec![
                    CheckedIntegerEndpointStep::Multiply(i128::MAX),
                    CheckedIntegerEndpointStep::Add(i128::MAX),
                ],
                ..form
            },
            &Proposition::LessOrEqual(root, literal(2)),
            &Proposition::LessOrEqual(target, literal(1)),
        ),
        Err(IntegerAffineBoundConversionError::MappedBoundOverflow),
    );
    assert_eq!(
        check_integer_affine_bound_conversion(
            &CheckedIntegerAffineForm {
                root: value(1, integer_type),
                target: value(2, integer_type),
                integer_type,
                coefficient: 2,
                offset: 1,
                endpoint_steps: vec![
                    CheckedIntegerEndpointStep::Multiply(2),
                    CheckedIntegerEndpointStep::Add(1),
                ],
            },
            &Proposition::LessOrEqual(value(1, integer_type), literal(100)),
            &Proposition::LessOrEqual(value(2, integer_type), literal(127)),
        ),
        Err(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier),
    );
}

#[test]
fn maps_mixed_exact_shift_endpoints_and_rejects_witness_tamper() {
    let value_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let root = value(1, value_type);
    let first = value(2, value_type);
    let second = value(3, value_type);
    let target = value(4, value_type);
    let landed_count = value(5, i8_type);
    let integer = |integer_type: IntegerType, value: i128| {
        let value = match integer_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(value),
            IntegerSign::Unsigned => IntegerValue::Unsigned(value.try_into().unwrap()),
        };
        ScalarTerm::integer(integer_type, value).unwrap()
    };
    let axioms = vec![
        Proposition::Equal(landed_count.clone(), integer(i8_type, 1)),
        Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_left(value_type, i8_type, root.clone(), landed_count)
                .unwrap(),
        ),
        Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_shift_right(
                value_type,
                u16_type,
                first,
                integer(u16_type, 2),
            )
            .unwrap(),
        ),
        Proposition::Equal(
            target.clone(),
            ScalarTerm::exact_integer_shift_left(
                value_type,
                i32_type,
                second,
                integer(i32_type, 3),
            )
            .unwrap(),
        ),
    ];
    let context = PropositionContext::from_value_types(
        (1..=4)
            .map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(value_type)))
            .chain([(ValueId::new(5).unwrap(), ScalarType::Integer(i8_type))]),
    )
    .unwrap();
    let witness = IntegerAffineWitness {
        root: root.clone(),
        target: target.clone(),
        definition_axioms: vec![1, 2, 3],
        literal_axioms: vec![Some(0), None, None],
    };
    let checked = check_integer_affine_witness(&context, &axioms, &witness)
        .expect("ordered mixed shift witness");
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::LessOrEqual(root.clone(), integer(value_type, 63)),
        ),
        Ok(Proposition::LessOrEqual(target, integer(value_type, 248),)),
    );
    assert_eq!(
        check_integer_affine_witness(
            &context,
            &axioms,
            &IntegerAffineWitness {
                definition_axioms: vec![1, 3, 2],
                ..witness.clone()
            },
        ),
        Err(IntegerAffineWitnessError::NonCanonicalDefinitionOrder),
    );
    assert_eq!(
        check_integer_affine_witness(
            &context,
            &axioms,
            &IntegerAffineWitness {
                literal_axioms: vec![None, None, None],
                ..witness
            },
        ),
        Err(IntegerAffineWitnessError::ShiftCountNotLanded(1)),
    );
}

#[test]
fn direct_add_bound_replays_both_ordered_endpoints_and_rejects_mutations() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = value(1, integer_type);
    let right = value(2, integer_type);
    let target = ScalarTerm::exact_integer_add(integer_type, left.clone(), right.clone())
        .expect("exact add");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(integer_type)),
    ])
    .unwrap();
    let checked = check_integer_affine_witness(
        &context,
        &[],
        &IntegerAffineWitness {
            root: left.clone(),
            target,
            definition_axioms: Vec::new(),
            literal_axioms: Vec::new(),
        },
    )
    .expect("direct mathematical add endpoint");
    let sum = IntegerMathTerm::Add(
        Box::new(IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: ValueId::new(1).unwrap(),
        }),
        Box::new(IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: ValueId::new(2).unwrap(),
        }),
    );
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![
                Proposition::LessOrEqual(literal(integer_type, -100), left.clone()),
                Proposition::LessOrEqual(literal(integer_type, 20), right.clone()),
            ]),
        ),
        Ok(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(IntegerValue::Signed(-80)),
            sum.clone(),
        )),
    );
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![
                Proposition::LessOrEqual(left.clone(), literal(integer_type, 100)),
                Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
            ]),
        ),
        Ok(Proposition::IntegerMathLessOrEqual(
            sum.clone(),
            IntegerMathTerm::literal(IntegerValue::Signed(120)),
        )),
    );
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![
                Proposition::Equal(left.clone(), literal(integer_type, 7)),
                Proposition::LessOrEqual(right.clone(), literal(integer_type, 100)),
            ]),
        ),
        Ok(Proposition::IntegerMathLessOrEqual(
            sum.clone(),
            IntegerMathTerm::literal(IntegerValue::Signed(107)),
        )),
    );
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![
                Proposition::Truth,
                Proposition::LessOrEqual(literal(integer_type, 0), right.clone()),
            ]),
        ),
        Ok(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(IntegerValue::Signed(-128)),
            sum.clone(),
        )),
    );
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![
                Proposition::Truth,
                Proposition::LessOrEqual(right.clone(), literal(integer_type, 0)),
            ]),
        ),
        Ok(Proposition::IntegerMathLessOrEqual(
            sum.clone(),
            IntegerMathTerm::literal(IntegerValue::Signed(127)),
        )),
    );
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![Proposition::Truth, Proposition::Truth]),
        ),
        Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch),
    );

    let seven = literal(integer_type, 7);
    for (target, root, evidence, expected_sum) in [
        (
            ScalarTerm::exact_integer_add(integer_type, left.clone(), seven.clone()).unwrap(),
            left.clone(),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(left.clone(), literal(integer_type, 100)),
                Proposition::Truth,
            ]),
            IntegerMathTerm::Add(
                Box::new(IntegerMathTerm::MathValue {
                    source_type: integer_type,
                    value: ValueId::new(1).unwrap(),
                }),
                Box::new(IntegerMathTerm::literal(IntegerValue::Signed(7))),
            ),
        ),
        (
            ScalarTerm::exact_integer_add(integer_type, seven.clone(), right.clone()).unwrap(),
            seven,
            Proposition::Conjunction(vec![
                Proposition::Truth,
                Proposition::LessOrEqual(right.clone(), literal(integer_type, 100)),
            ]),
            IntegerMathTerm::Add(
                Box::new(IntegerMathTerm::literal(IntegerValue::Signed(7))),
                Box::new(IntegerMathTerm::MathValue {
                    source_type: integer_type,
                    value: ValueId::new(2).unwrap(),
                }),
            ),
        ),
    ] {
        let literal_checked = check_integer_affine_witness(
            &context,
            &[],
            &IntegerAffineWitness {
                root,
                target,
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        )
        .expect("direct add with embedded literal");
        assert_eq!(
            map_integer_affine_bound(&literal_checked, &evidence),
            Ok(Proposition::IntegerMathLessOrEqual(
                expected_sum,
                IntegerMathTerm::literal(IntegerValue::Signed(107)),
            )),
        );
    }

    for malformed in [
        Proposition::LessOrEqual(left.clone(), literal(integer_type, 100)),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(literal(integer_type, -100), left.clone()),
            Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
        ]),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(value(3, integer_type), literal(integer_type, 100)),
            Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
        ]),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
            Proposition::LessOrEqual(left.clone(), literal(integer_type, 100)),
        ]),
    ] {
        assert_eq!(
            map_integer_affine_bound(&checked, &malformed),
            Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch),
        );
    }

    let i128_type = IntegerType::new(IntegerSign::Signed, 128).expect("i128");
    let i128_left = value(4, i128_type);
    let i128_right = value(5, i128_type);
    let i128_context = PropositionContext::from_value_types([
        (ValueId::new(4).unwrap(), ScalarType::Integer(i128_type)),
        (ValueId::new(5).unwrap(), ScalarType::Integer(i128_type)),
    ])
    .unwrap();
    let i128_checked = check_integer_affine_witness(
        &i128_context,
        &[],
        &IntegerAffineWitness {
            root: i128_left.clone(),
            target: ScalarTerm::exact_integer_add(i128_type, i128_left.clone(), i128_right.clone())
                .unwrap(),
            definition_axioms: Vec::new(),
            literal_axioms: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(
        map_integer_affine_bound(
            &i128_checked,
            &Proposition::Conjunction(vec![
                Proposition::LessOrEqual(literal(i128_type, i128::MIN), i128_left,),
                Proposition::LessOrEqual(literal(i128_type, i128::MIN), i128_right,),
            ]),
        ),
        Err(IntegerAffineBoundConversionError::DirectAddBoundOverflow),
    );
}

#[test]
fn correlated_add_bound_replays_exact_complement_identity_and_relation() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = value(1, integer_type);
    let right = value(2, integer_type);
    let correlated = value(3, integer_type);
    let redirected = value(4, integer_type);
    let target = ScalarTerm::exact_integer_add(integer_type, left.clone(), right.clone())
        .expect("exact add");
    let context = PropositionContext::from_value_types(
        (1..=5).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    for (endpoint, evidence, expected) in [
        (
            127,
            Proposition::LessOrEqual(left.clone(), correlated.clone()),
            Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Add(
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(1).unwrap(),
                    }),
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(2).unwrap(),
                    }),
                ),
                IntegerMathTerm::literal(IntegerValue::Signed(127)),
            ),
        ),
        (
            -128,
            Proposition::LessOrEqual(correlated.clone(), left.clone()),
            Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Signed(-128)),
                IntegerMathTerm::Add(
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(1).unwrap(),
                    }),
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(2).unwrap(),
                    }),
                ),
            ),
        ),
    ] {
        let axiom = Proposition::Equal(
            correlated.clone(),
            ScalarTerm::exact_integer_subtract(
                integer_type,
                literal(integer_type, endpoint),
                right.clone(),
            )
            .unwrap(),
        );
        let witness = IntegerAffineWitness {
            root: correlated.clone(),
            target: target.clone(),
            definition_axioms: vec![0],
            literal_axioms: vec![None],
        };
        let checked =
            check_integer_affine_witness(&context, std::slice::from_ref(&axiom), &witness)
                .expect("correlated add witness");
        assert_eq!(map_integer_affine_bound(&checked, &evidence), Ok(expected));
        assert_eq!(
            map_integer_affine_bound(
                &checked,
                &Proposition::LessOrEqual(left.clone(), redirected.clone()),
            ),
            Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch),
        );
        assert!(
            check_integer_affine_witness(
                &context,
                &[Proposition::Equal(
                    correlated.clone(),
                    ScalarTerm::exact_integer_subtract(
                        integer_type,
                        literal(integer_type, endpoint),
                        redirected.clone(),
                    )
                    .unwrap(),
                )],
                &witness,
            )
            .is_err(),
            "redirecting the complement operand invalidates the witness",
        );
        assert!(
            check_integer_affine_witness(
                &context,
                std::slice::from_ref(&axiom),
                &IntegerAffineWitness {
                    definition_axioms: Vec::new(),
                    literal_axioms: Vec::new(),
                    ..witness
                },
            )
            .is_err(),
            "omitting correlated definition custody invalidates the witness",
        );
    }

    let landed_endpoint = value(5, integer_type);
    let landing = Proposition::Equal(landed_endpoint.clone(), literal(integer_type, 127));
    let complement = Proposition::Equal(
        correlated.clone(),
        ScalarTerm::exact_integer_subtract(integer_type, landed_endpoint.clone(), right.clone())
            .unwrap(),
    );
    let landed_witness = IntegerAffineWitness {
        root: correlated.clone(),
        target: target.clone(),
        definition_axioms: vec![1],
        literal_axioms: vec![Some(0)],
    };
    let landed = check_integer_affine_witness(
        &context,
        &[landing.clone(), complement.clone()],
        &landed_witness,
    )
    .expect("landed carrier endpoint and complement are replayed in order");
    assert_eq!(
        map_integer_affine_bound(
            &landed,
            &Proposition::LessOrEqual(left.clone(), correlated.clone()),
        ),
        Ok(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Add(
                Box::new(IntegerMathTerm::MathValue {
                    source_type: integer_type,
                    value: ValueId::new(1).unwrap(),
                }),
                Box::new(IntegerMathTerm::MathValue {
                    source_type: integer_type,
                    value: ValueId::new(2).unwrap(),
                }),
            ),
            IntegerMathTerm::literal(IntegerValue::Signed(127)),
        )),
    );
    let redirected_complement = Proposition::Equal(
        correlated.clone(),
        ScalarTerm::exact_integer_subtract(
            integer_type,
            landed_endpoint.clone(),
            redirected.clone(),
        )
        .unwrap(),
    );
    for (mutation, (axioms, witness)) in [
        (
            vec![landing.clone(), complement.clone()],
            IntegerAffineWitness {
                literal_axioms: vec![None],
                ..landed_witness.clone()
            },
        ),
        (
            vec![complement.clone(), landing.clone()],
            IntegerAffineWitness {
                definition_axioms: vec![0],
                literal_axioms: vec![Some(1)],
                ..landed_witness.clone()
            },
        ),
        (
            vec![
                Proposition::Equal(landed_endpoint.clone(), literal(integer_type, 126)),
                complement.clone(),
            ],
            landed_witness.clone(),
        ),
        (
            vec![
                Proposition::Equal(landed_endpoint.clone(), ScalarTerm::boolean(true)),
                complement.clone(),
            ],
            landed_witness.clone(),
        ),
        (
            vec![
                Proposition::Equal(redirected, literal(integer_type, 127)),
                complement,
            ],
            landed_witness.clone(),
        ),
        (vec![landing, redirected_complement], landed_witness),
    ]
    .into_iter()
    .enumerate()
    {
        let result = check_integer_affine_witness(&context, &axioms, &witness);
        assert!(
            result.is_err(),
            "reordered, omitted, or stale endpoint landing custody rejects: {mutation}: {result:?}",
        );
    }
}

#[test]
fn direct_subtract_bound_replays_opposite_ordered_endpoints_and_rejects_mutations() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = value(1, integer_type);
    let right = value(2, integer_type);
    let target =
        ScalarTerm::exact_integer_subtract(integer_type, left.clone(), right.clone()).unwrap();
    let context = PropositionContext::from_value_types(
        (1..=5).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    let checked = check_integer_affine_witness(
        &context,
        &[],
        &IntegerAffineWitness {
            root: left.clone(),
            target,
            definition_axioms: Vec::new(),
            literal_axioms: Vec::new(),
        },
    )
    .expect("direct subtract witness");
    let lower = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(literal(integer_type, -100), left.clone()),
        Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
    ]);
    assert_eq!(
        map_integer_affine_bound(&checked, &lower),
        Ok(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(IntegerValue::Signed(-120)),
            IntegerMathTerm::Subtract(
                Box::new(IntegerMathTerm::MathValue {
                    source_type: integer_type,
                    value: ValueId::new(1).unwrap(),
                }),
                Box::new(IntegerMathTerm::MathValue {
                    source_type: integer_type,
                    value: ValueId::new(2).unwrap(),
                }),
            ),
        )),
    );
    let upper = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(left.clone(), literal(integer_type, 100)),
        Proposition::LessOrEqual(literal(integer_type, -20), right.clone()),
    ]);
    assert!(map_integer_affine_bound(&checked, &upper).is_ok());
    for malformed in [
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(literal(integer_type, -100), left.clone()),
            Proposition::LessOrEqual(literal(integer_type, -20), right.clone()),
        ]),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
            Proposition::LessOrEqual(literal(integer_type, -100), left.clone()),
        ]),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(literal(integer_type, -100), value(3, integer_type)),
            Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
        ]),
    ] {
        assert!(
            map_integer_affine_bound(&checked, &malformed).is_err(),
            "mixed orientation, reordered evidence, and redirected operands reject",
        );
    }
    assert!(
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![
                Proposition::Equal(left.clone(), literal(integer_type, 127)),
                Proposition::Truth,
            ]),
        )
        .is_ok(),
        "an exact MAX minuend and carrier-wide subtrahend derive the lower endpoint",
    );
    assert!(
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![
                Proposition::Equal(left, literal(integer_type, 100)),
                Proposition::Truth,
            ]),
        )
        .is_err(),
        "a noncarrier exact minuend cannot orient bare subtrahend inclusion",
    );
}

#[test]
fn correlated_subtract_replays_complement_definition_and_endpoint_landing() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = value(1, integer_type);
    let right = value(2, integer_type);
    let correlated = value(3, integer_type);
    let landed_endpoint = value(4, integer_type);
    let target =
        ScalarTerm::exact_integer_subtract(integer_type, left.clone(), right.clone()).unwrap();
    let context = PropositionContext::from_value_types(
        (1..=5).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    let landing = Proposition::Equal(landed_endpoint.clone(), literal(integer_type, -128));
    let complement = Proposition::Equal(
        correlated.clone(),
        ScalarTerm::exact_integer_add(integer_type, landed_endpoint.clone(), right.clone())
            .unwrap(),
    );
    let witness = IntegerAffineWitness {
        root: correlated.clone(),
        target: target.clone(),
        definition_axioms: vec![1],
        literal_axioms: vec![Some(0)],
    };
    let checked =
        check_integer_affine_witness(&context, &[landing.clone(), complement.clone()], &witness)
            .expect("landed MIN plus right complement");
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::LessOrEqual(correlated.clone(), left.clone()),
        ),
        Ok(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(IntegerValue::Signed(-128)),
            IntegerMathTerm::Subtract(
                Box::new(IntegerMathTerm::MathValue {
                    source_type: integer_type,
                    value: ValueId::new(1).unwrap(),
                }),
                Box::new(IntegerMathTerm::MathValue {
                    source_type: integer_type,
                    value: ValueId::new(2).unwrap(),
                }),
            ),
        )),
    );
    assert!(
        check_integer_affine_witness(
            &context,
            &[landing.clone(), complement.clone()],
            &IntegerAffineWitness {
                literal_axioms: vec![None],
                ..witness.clone()
            },
        )
        .is_err(),
        "omitting the endpoint landing rejects",
    );
    let stale = Proposition::Equal(value(5, integer_type), literal(integer_type, -128));
    assert!(
        check_integer_affine_witness(&context, &[stale, complement], &witness).is_err(),
        "redirecting the endpoint landing rejects",
    );
    assert!(
        map_integer_affine_bound(&checked, &Proposition::LessOrEqual(left, correlated),).is_err(),
        "reversing the authored guard rejects",
    );
}

#[test]
fn unsigned_correlated_subtract_replays_exact_operand_order() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = value(1, integer_type);
    let right = value(2, integer_type);
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
    ])
    .unwrap();
    let checked = check_integer_affine_witness(
        &context,
        &[],
        &IntegerAffineWitness {
            root: right.clone(),
            target: ScalarTerm::exact_integer_subtract(integer_type, left.clone(), right.clone())
                .unwrap(),
            definition_axioms: Vec::new(),
            literal_axioms: Vec::new(),
        },
    )
    .expect("unsigned joint guard witness");
    assert!(
        map_integer_affine_bound(
            &checked,
            &Proposition::LessOrEqual(right.clone(), left.clone()),
        )
        .is_ok(),
    );
    assert!(
        map_integer_affine_bound(&checked, &Proposition::LessOrEqual(left, right)).is_err(),
        "reversed unsigned guard rejects",
    );
}

#[test]
fn direct_multiply_replays_four_corners_and_rejects_order_mutations() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = value(1, integer_type);
    let right = value(2, integer_type);
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(integer_type)),
    ])
    .unwrap();
    let checked = check_integer_affine_witness(
        &context,
        &[],
        &IntegerAffineWitness {
            root: left.clone(),
            target: ScalarTerm::exact_integer_multiply(integer_type, left.clone(), right.clone())
                .unwrap(),
            definition_axioms: Vec::new(),
            literal_axioms: Vec::new(),
        },
    )
    .expect("direct multiply witness");
    let lower = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(literal(integer_type, -4), left.clone()),
        Proposition::LessOrEqual(left.clone(), literal(integer_type, 5)),
        Proposition::LessOrEqual(literal(integer_type, -3), right.clone()),
        Proposition::LessOrEqual(right.clone(), literal(integer_type, 2)),
    ]);
    let product = IntegerMathTerm::Multiply(
        Box::new(IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: ValueId::new(1).unwrap(),
        }),
        Box::new(IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: ValueId::new(2).unwrap(),
        }),
    );
    assert_eq!(
        map_integer_affine_bound(&checked, &lower),
        Ok(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(IntegerValue::Signed(-15)),
            product,
        )),
    );
    for malformed in [
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(left.clone(), literal(integer_type, 5)),
            Proposition::LessOrEqual(literal(integer_type, -4), left.clone()),
            Proposition::LessOrEqual(literal(integer_type, -3), right.clone()),
            Proposition::LessOrEqual(right.clone(), literal(integer_type, 2)),
        ]),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(literal(integer_type, -4), left.clone()),
            Proposition::LessOrEqual(left.clone(), literal(integer_type, 5)),
            Proposition::LessOrEqual(literal(integer_type, -3), value(3, integer_type)),
            Proposition::LessOrEqual(right.clone(), literal(integer_type, 2)),
        ]),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(literal(integer_type, -4), left.clone()),
            Proposition::LessOrEqual(left, literal(integer_type, 5)),
            Proposition::LessOrEqual(literal(integer_type, -3), right),
        ]),
    ] {
        assert!(map_integer_affine_bound(&checked, &malformed).is_err());
    }
}

#[test]
fn direct_multiply_requires_oriented_zero_evidence_for_exact_zero_bounds() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = value(1, integer_type);
    let right = value(2, integer_type);
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
    ])
    .unwrap();
    let checked = check_integer_affine_witness(
        &context,
        &[],
        &IntegerAffineWitness {
            root: left.clone(),
            target: ScalarTerm::exact_integer_multiply(integer_type, left.clone(), right.clone())
                .unwrap(),
            definition_axioms: Vec::new(),
            literal_axioms: Vec::new(),
        },
    )
    .expect("direct multiply witness");
    let zero = literal(integer_type, 0);
    let product = IntegerMathTerm::Multiply(
        Box::new(IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: ValueId::new(1).unwrap(),
        }),
        Box::new(IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: ValueId::new(2).unwrap(),
        }),
    );
    let lower = Proposition::Conjunction(vec![
        Proposition::Truth,
        Proposition::Truth,
        Proposition::LessOrEqual(zero.clone(), right.clone()),
        Proposition::LessOrEqual(right.clone(), zero.clone()),
    ]);
    assert_eq!(
        map_integer_affine_bound(&checked, &lower),
        Ok(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(IntegerValue::Signed(0)),
            product.clone(),
        )),
    );
    let upper = Proposition::Conjunction(vec![
        Proposition::Truth,
        Proposition::Truth,
        Proposition::LessOrEqual(right.clone(), zero.clone()),
        Proposition::LessOrEqual(zero, right.clone()),
    ]);
    assert_eq!(
        map_integer_affine_bound(&checked, &upper),
        Ok(Proposition::IntegerMathLessOrEqual(
            product,
            IntegerMathTerm::literal(IntegerValue::Signed(0)),
        )),
    );

    let one_equality = Proposition::Equal(right, literal(integer_type, 1));
    let unoriented_nonzero = Proposition::Conjunction(vec![
        Proposition::Truth,
        Proposition::Truth,
        one_equality.clone(),
        one_equality,
    ]);
    assert!(map_integer_affine_bound(&checked, &unoriented_nonzero).is_err());
}

#[test]
fn correlated_negative_multiply_uses_target_endpoint_and_sign_orientation() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = value(1, integer_type);
    let right = value(2, integer_type);
    let quotient = value(3, integer_type);
    let context = PropositionContext::from_value_types(
        (1..=5).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    let target =
        ScalarTerm::exact_integer_multiply(integer_type, left.clone(), right.clone()).unwrap();
    let negative = Proposition::LessOrEqual(right.clone(), literal(integer_type, -2));
    for (endpoint, comparison, expected) in [
        (
            -128,
            Proposition::LessOrEqual(left.clone(), quotient.clone()),
            Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Signed(-128)),
                IntegerMathTerm::Multiply(
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(1).unwrap(),
                    }),
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(2).unwrap(),
                    }),
                ),
            ),
        ),
        (
            127,
            Proposition::LessOrEqual(quotient.clone(), left.clone()),
            Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Multiply(
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(1).unwrap(),
                    }),
                    Box::new(IntegerMathTerm::MathValue {
                        source_type: integer_type,
                        value: ValueId::new(2).unwrap(),
                    }),
                ),
                IntegerMathTerm::literal(IntegerValue::Signed(127)),
            ),
        ),
    ] {
        let axiom = Proposition::Equal(
            quotient.clone(),
            ScalarTerm::exact_integer_divide(
                integer_type,
                literal(integer_type, endpoint),
                right.clone(),
            )
            .unwrap(),
        );
        let checked = check_integer_affine_witness(
            &context,
            std::slice::from_ref(&axiom),
            &IntegerAffineWitness {
                root: quotient.clone(),
                target: target.clone(),
                definition_axioms: vec![0],
                literal_axioms: vec![None],
            },
        )
        .expect("negative quotient witness");
        let evidence = Proposition::Conjunction(vec![negative.clone(), comparison.clone()]);
        assert_eq!(map_integer_affine_bound(&checked, &evidence), Ok(expected));
        let Proposition::LessOrEqual(comparison_left, comparison_right) = &comparison else {
            unreachable!("correlated comparison is ordered")
        };
        assert_eq!(
            map_integer_affine_bound(
                &checked,
                &Proposition::Conjunction(vec![
                    negative.clone(),
                    Proposition::LessOrEqual(comparison_right.clone(), comparison_left.clone(),),
                ]),
            ),
            Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch),
            "for right=-2, lower uses left<=MIN/right (64) and upper uses MAX/right (-63)<=left",
        );
    }

    let landed_endpoint = value(4, integer_type);
    let landing = Proposition::Equal(landed_endpoint.clone(), literal(integer_type, -128));
    let definition = Proposition::Equal(
        quotient.clone(),
        ScalarTerm::exact_integer_divide(integer_type, landed_endpoint.clone(), right.clone())
            .unwrap(),
    );
    let landed_witness = IntegerAffineWitness {
        root: quotient,
        target,
        definition_axioms: vec![1],
        literal_axioms: vec![Some(0)],
    };
    assert!(
        check_integer_affine_witness(
            &context,
            &[landing.clone(), definition.clone()],
            &landed_witness,
        )
        .is_ok(),
        "an exact earlier endpoint landing is replayed",
    );
    for (axioms, witness) in [
        (
            vec![landing.clone(), definition.clone()],
            IntegerAffineWitness {
                literal_axioms: vec![None],
                ..landed_witness.clone()
            },
        ),
        (
            vec![definition.clone(), landing.clone()],
            IntegerAffineWitness {
                definition_axioms: vec![0],
                literal_axioms: vec![Some(1)],
                ..landed_witness.clone()
            },
        ),
        (
            vec![
                Proposition::Equal(landed_endpoint.clone(), literal(integer_type, -127)),
                definition.clone(),
            ],
            landed_witness.clone(),
        ),
        (
            vec![
                landing,
                Proposition::Equal(
                    landed_witness.root.clone(),
                    ScalarTerm::exact_integer_divide(
                        integer_type,
                        landed_endpoint,
                        value(5, integer_type),
                    )
                    .unwrap(),
                ),
            ],
            landed_witness,
        ),
    ] {
        assert!(check_integer_affine_witness(&context, &axioms, &witness).is_err());
    }
}

#[test]
fn direct_shift_bound_replays_count_range_and_sign_oriented_endpoint() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
    let root = value(1, integer_type);
    let count = value(2, count_type);
    let target =
        ScalarTerm::exact_integer_shift_left(integer_type, count_type, root.clone(), count.clone())
            .unwrap();
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(count_type)),
    ])
    .unwrap();
    let checked = check_integer_affine_witness(
        &context,
        &[],
        &IntegerAffineWitness {
            root: root.clone(),
            target,
            definition_axioms: Vec::new(),
            literal_axioms: Vec::new(),
        },
    )
    .expect("direct mathematical shift endpoint");
    let lower_count = Proposition::LessOrEqual(literal(count_type, 0), count.clone());
    let upper_count = Proposition::LessOrEqual(count.clone(), literal(count_type, 3));
    let shifted = IntegerMathTerm::ShiftLeft {
        value: Box::new(IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: ValueId::new(1).unwrap(),
        }),
        count: Box::new(IntegerMathTerm::MathValue {
            source_type: count_type,
            value: ValueId::new(2).unwrap(),
        }),
    };
    let mapped = |root_bound| {
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![root_bound, lower_count.clone(), upper_count.clone()]),
        )
    };
    assert_eq!(
        mapped(Proposition::LessOrEqual(
            literal(integer_type, -16),
            root.clone(),
        )),
        Ok(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(IntegerValue::Signed(-128)),
            shifted.clone(),
        )),
    );
    assert_eq!(
        mapped(Proposition::LessOrEqual(
            root.clone(),
            literal(integer_type, 15),
        )),
        Ok(Proposition::IntegerMathLessOrEqual(
            shifted.clone(),
            IntegerMathTerm::literal(IntegerValue::Signed(120)),
        )),
    );
    assert_eq!(
        mapped(Proposition::LessOrEqual(
            literal(integer_type, 2),
            root.clone(),
        )),
        Ok(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(IntegerValue::Signed(2)),
            shifted.clone(),
        )),
    );
    assert_eq!(
        mapped(Proposition::LessOrEqual(
            root.clone(),
            literal(integer_type, -2),
        )),
        Ok(Proposition::IntegerMathLessOrEqual(
            shifted,
            IntegerMathTerm::literal(IntegerValue::Signed(-2)),
        )),
    );
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![
                Proposition::LessOrEqual(root.clone(), literal(integer_type, 15)),
                upper_count,
            ]),
        ),
        Err(IntegerAffineBoundConversionError::DirectShiftCountLowerMissing),
    );
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![
                Proposition::LessOrEqual(root, literal(integer_type, 15)),
                lower_count,
                Proposition::LessOrEqual(count, literal(count_type, 8)),
            ]),
        ),
        Err(IntegerAffineBoundConversionError::DirectShiftCountOutsideValueWidth),
    );
}

#[test]
fn bitwise_and_mask_maps_every_root_endpoint_to_the_mask_image() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let root = value(1, integer_type);
    let target = value(2, integer_type);
    let literal = |value| literal(integer_type, value);
    let context = PropositionContext::from_value_types(
        (1..=2).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    let definition = Proposition::Equal(
        target.clone(),
        ScalarTerm::integer_bitwise_and(integer_type, root.clone(), literal(15)).unwrap(),
    );
    let witness = IntegerAffineWitness {
        root: root.clone(),
        target: target.clone(),
        definition_axioms: vec![0],
        literal_axioms: vec![None],
    };
    let checked =
        check_integer_affine_witness(&context, std::slice::from_ref(&definition), &witness)
            .expect("bitwise-and mask witness");

    // The mask image is total: `Truth` yields both endpoints of `[0, 15]`.
    assert_eq!(
        integer_affine_truth_bounds(&checked),
        Ok(vec![
            Proposition::LessOrEqual(literal(0), target.clone()),
            Proposition::LessOrEqual(target.clone(), literal(15)),
        ]),
    );

    // An upper root bound maps to the mask, not to its own literal:
    // `c <= 259` becomes `target <= 15`.
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::LessOrEqual(root.clone(), literal(259)),
        ),
        Ok(Proposition::LessOrEqual(target.clone(), literal(15))),
    );
    // A lower root bound collapses to zero; even `5 <= c` cannot keep the
    // literal since masking drops operand bits.
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::LessOrEqual(literal(5), root.clone()),
        ),
        Ok(Proposition::LessOrEqual(literal(0), target.clone())),
    );
    // A strict bound is not a translation through a mask and rejects.
    assert_eq!(
        map_integer_affine_bound(&checked, &Proposition::LessThan(root.clone(), literal(259)),),
        Err(IntegerAffineBoundConversionError::StrictBoundNotTranslation),
    );
    // A conclusion that does not match the image rejects.
    assert_eq!(
        check_integer_affine_bound_conversion(
            &checked,
            &Proposition::LessOrEqual(root.clone(), literal(259)),
            &Proposition::LessOrEqual(target.clone(), literal(16)),
        ),
        Err(IntegerAffineBoundConversionError::ConclusionMismatch),
    );
    assert_eq!(
        check_integer_affine_bound_conversion(
            &checked,
            &Proposition::LessOrEqual(literal(0), root.clone()),
            &Proposition::LessOrEqual(literal(1), target.clone()),
        ),
        Err(IntegerAffineBoundConversionError::ConclusionMismatch),
    );

    // The same step inside a longer chain keeps mapping through later
    // translations: `v = c & 15; w = v - 3` sends `c <= 259` to `w <= 12`.
    let widened = value(3, integer_type);
    let context = PropositionContext::from_value_types(
        (1..=3).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    let axioms = [
        definition,
        Proposition::Equal(
            widened.clone(),
            ScalarTerm::exact_integer_subtract(integer_type, target.clone(), literal(3)).unwrap(),
        ),
    ];
    let chained = check_integer_affine_witness(
        &context,
        &axioms,
        &IntegerAffineWitness {
            root: root.clone(),
            target: widened.clone(),
            definition_axioms: vec![0, 1],
            literal_axioms: vec![None, None],
        },
    )
    .expect("mask then subtract chain");
    assert_eq!(
        map_integer_affine_bound(
            &chained,
            &Proposition::LessOrEqual(root.clone(), literal(259)),
        ),
        Ok(Proposition::LessOrEqual(widened.clone(), literal(12))),
    );
    assert_eq!(
        map_integer_affine_bound(&chained, &Proposition::LessOrEqual(literal(0), root),),
        Ok(Proposition::LessOrEqual(literal(-3), widened)),
    );
}
