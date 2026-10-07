use super::*;

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
