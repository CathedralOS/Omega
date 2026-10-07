use super::{literal, unsigned_literal, value};
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

mod correlated;
mod direct;

#[test]

fn unsigned_remainder_intersects_dividend_bounds_and_accepts_legacy_images() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let root = value(1, integer_type);
    let target = value(2, integer_type);
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
    ])
    .unwrap();
    let literal = |number| unsigned_literal(integer_type, number);
    let definition = Proposition::Equal(
        target.clone(),
        ScalarTerm::exact_integer_remainder(integer_type, root.clone(), literal(256)).unwrap(),
    );
    let form = check_integer_affine_witness(
        &context,
        &[definition],
        &IntegerAffineWitness {
            root: root.clone(),
            target: target.clone(),
            definition_axioms: vec![0],
            literal_axioms: vec![None],
        },
    )
    .unwrap();
    for (bound, expected) in [
        (0, 0),
        (57, 57),
        (255, 255),
        (256, 255),
        (u32::MAX as u128, 255),
    ] {
        let premise = Proposition::LessOrEqual(root.clone(), literal(bound));
        let conclusion = Proposition::LessOrEqual(target.clone(), literal(expected));
        assert_eq!(
            map_integer_affine_bound(&form, &premise),
            Ok(conclusion.clone())
        );
        assert_eq!(
            check_integer_affine_bound_conversion(&form, &premise, &conclusion),
            Ok(())
        );
        assert_eq!(
            check_integer_affine_bound_conversion(
                &form,
                &premise,
                &Proposition::LessOrEqual(target.clone(), literal(255)),
            ),
            Ok(()),
            "the previous total-image certificate remains valid",
        );
    }
    for wrong in [56, 58] {
        assert_eq!(
            check_integer_affine_bound_conversion(
                &form,
                &Proposition::LessOrEqual(root.clone(), literal(57)),
                &Proposition::LessOrEqual(target.clone(), literal(wrong)),
            ),
            Err(IntegerAffineBoundConversionError::ConclusionMismatch),
            "only the tight or legacy mapping is admitted; relaxation needs evidence",
        );
    }
    assert_eq!(
        map_integer_affine_bound(&form, &Proposition::LessOrEqual(literal(57), root.clone())),
        Ok(Proposition::LessOrEqual(literal(0), target)),
    );
    assert_eq!(
        map_integer_affine_bound(&form, &Proposition::LessThan(root, literal(57))),
        Err(IntegerAffineBoundConversionError::StrictBoundNotTranslation),
    );
}

#[test]
fn signed_remainder_keeps_truncation_bounds_for_both_divisor_signs() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 16).unwrap();
    let root = value(1, integer_type);
    let target = value(2, integer_type);
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
    ])
    .unwrap();
    for divisor in [-256, 256] {
        let definition = Proposition::Equal(
            target.clone(),
            ScalarTerm::exact_integer_remainder(
                integer_type,
                root.clone(),
                literal(integer_type, divisor),
            )
            .unwrap(),
        );
        let form = check_integer_affine_witness(
            &context,
            &[definition],
            &IntegerAffineWitness {
                root: root.clone(),
                target: target.clone(),
                definition_axioms: vec![0],
                literal_axioms: vec![None],
            },
        )
        .unwrap();
        for (bound, upper, expected) in [
            (-57, true, 0),
            (0, true, 0),
            (57, true, 255),
            (-57, false, -255),
            (0, false, 0),
            (57, false, 0),
        ] {
            let premise = if upper {
                Proposition::LessOrEqual(root.clone(), literal(integer_type, bound))
            } else {
                Proposition::LessOrEqual(literal(integer_type, bound), root.clone())
            };
            let conclusion = if upper {
                Proposition::LessOrEqual(target.clone(), literal(integer_type, expected))
            } else {
                Proposition::LessOrEqual(literal(integer_type, expected), target.clone())
            };
            assert_eq!(map_integer_affine_bound(&form, &premise), Ok(conclusion));
        }
    }
}

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

#[test]
fn saturating_endpoints_cover_every_byte_input_and_reject_plateau_strictness() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let integer_type = IntegerType::new(sign, 8).unwrap();
        let (minimum, maximum): (i128, i128) = match sign {
            IntegerSign::Signed => (-128, 127),
            IntegerSign::Unsigned => (0, 255),
        };
        let integer = |n: i128| match sign {
            IntegerSign::Signed => literal(integer_type, n),
            IntegerSign::Unsigned => unsigned_literal(integer_type, n as u128),
        };
        let root = value(1, integer_type);
        let target = value(2, integer_type);
        let context = PropositionContext::from_value_types(
            (1..=3).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
        )
        .unwrap();
        // Include both carrier edges, signed negative literals, subtraction,
        // and commuted addition. Check actual byte semantics independently of
        // the certificate's endpoint implementation.
        for amount in [minimum, 1, maximum] {
            for operation in 0..3 {
                let expression = match operation {
                    0 => ScalarTerm::saturating_integer_add(
                        integer_type,
                        root.clone(),
                        integer(amount),
                    ),
                    1 => ScalarTerm::saturating_integer_add(
                        integer_type,
                        integer(amount),
                        root.clone(),
                    ),
                    _ => ScalarTerm::saturating_integer_subtract(
                        integer_type,
                        root.clone(),
                        integer(amount),
                    ),
                }
                .unwrap();
                for inline in [false, true] {
                    let axioms = if inline {
                        vec![]
                    } else {
                        vec![Proposition::Equal(target.clone(), expression.clone())]
                    };
                    let result = if inline {
                        expression.clone()
                    } else {
                        target.clone()
                    };
                    let witness = IntegerAffineWitness {
                        root: root.clone(),
                        target: result.clone(),
                        definition_axioms: if inline { vec![] } else { vec![0] },
                        literal_axioms: if inline { vec![] } else { vec![None] },
                    };
                    let checked =
                        check_integer_affine_witness(&context, &axioms, &witness).unwrap();
                    let execute = |input: i128| match (sign, operation) {
                        (IntegerSign::Signed, 2) => {
                            (input as i8).saturating_sub(amount as i8) as i128
                        }
                        (IntegerSign::Signed, _) => {
                            (input as i8).saturating_add(amount as i8) as i128
                        }
                        (IntegerSign::Unsigned, 2) => {
                            (input as u8).saturating_sub(amount as u8) as i128
                        }
                        (IntegerSign::Unsigned, _) => {
                            (input as u8).saturating_add(amount as u8) as i128
                        }
                    };
                    for bound in minimum..=maximum {
                        for upper in [false, true] {
                            let premise = if upper {
                                Proposition::LessOrEqual(root.clone(), integer(bound))
                            } else {
                                Proposition::LessOrEqual(integer(bound), root.clone())
                            };
                            let endpoint = execute(bound);
                            let conclusion = if upper {
                                Proposition::LessOrEqual(result.clone(), integer(endpoint))
                            } else {
                                Proposition::LessOrEqual(integer(endpoint), result.clone())
                            };
                            assert_eq!(
                                map_integer_affine_bound(&checked, &premise),
                                Ok(conclusion.clone())
                            );
                            assert_eq!(
                                check_integer_affine_bound_conversion(
                                    &checked,
                                    &premise,
                                    &conclusion
                                ),
                                Ok(())
                            );
                            for input in minimum..=maximum {
                                if upper && input <= bound {
                                    assert!(execute(input) <= endpoint);
                                }
                                if !upper && bound <= input {
                                    assert!(endpoint <= execute(input));
                                }
                            }
                        }
                    }
                    assert_eq!(
                        map_integer_affine_bound(
                            &checked,
                            &Proposition::LessThan(root.clone(), integer(maximum))
                        ),
                        Err(IntegerAffineBoundConversionError::StrictBoundNotTranslation),
                    );
                    assert_eq!(
                        integer_affine_truth_bounds(&checked),
                        Ok(vec![
                            Proposition::LessOrEqual(integer(execute(minimum)), result.clone()),
                            Proposition::LessOrEqual(result.clone(), integer(execute(maximum))),
                        ])
                    );
                    if !inline {
                        // Saturation is many-to-one: reading this defining row
                        // backwards must not recover an exact input bound.
                        assert!(
                            check_integer_affine_witness(
                                &context,
                                &axioms,
                                &IntegerAffineWitness {
                                    root: target.clone(),
                                    target: root.clone(),
                                    ..witness.clone()
                                }
                            )
                            .is_err()
                        );
                        let changed = Proposition::Equal(
                            target.clone(),
                            ScalarTerm::saturating_integer_add(
                                integer_type,
                                value(3, integer_type),
                                integer(amount),
                            )
                            .unwrap(),
                        );
                        assert!(
                            check_integer_affine_witness(&context, &[changed], &witness).is_err()
                        );
                    }
                }
            }
        }
    }
}
