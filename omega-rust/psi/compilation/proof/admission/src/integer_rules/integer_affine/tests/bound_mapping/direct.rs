use super::*;

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
