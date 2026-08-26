use super::super::*;
use psi_core::{IntegerType, ScalarType, ValueId};

#[test]
fn exact_multiply_reconstructs_carrier_tight_known_factor_bounds() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let unsigned_value = ScalarTerm::value(
        ValueId::new(3).expect("value"),
        ScalarType::Integer(u8_type),
    );
    let unsigned_five = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(5)).expect("5u8");
    let unsigned_one = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).expect("1u8");
    let unsigned_maximum = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(51)).expect("51u8");
    assert_eq!(
        exact_integer_multiply_obligation(u8_type, unsigned_value.clone(), unsigned_one, &[],),
        Proposition::Truth
    );
    assert_eq!(
        exact_integer_multiply_obligation(
            u8_type,
            unsigned_value.clone(),
            unsigned_five.clone(),
            &[],
        ),
        Proposition::LessOrEqual(unsigned_value.clone(), unsigned_maximum.clone())
    );
    assert_eq!(
        exact_integer_multiply_obligation(u8_type, unsigned_five, unsigned_value.clone(), &[],),
        Proposition::LessOrEqual(unsigned_value, unsigned_maximum)
    );

    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_value = ScalarTerm::value(
        ValueId::new(4).expect("value"),
        ScalarType::Integer(i8_type),
    );
    let signed_three = ScalarTerm::integer(i8_type, IntegerValue::Signed(3)).expect("3i8");
    let negative_three = ScalarTerm::integer(i8_type, IntegerValue::Signed(-3)).expect("-3i8");
    let negative_42 = ScalarTerm::integer(i8_type, IntegerValue::Signed(-42)).expect("-42i8");
    let positive_42 = ScalarTerm::integer(i8_type, IntegerValue::Signed(42)).expect("42i8");
    let expected = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(negative_42.clone(), signed_value.clone()),
        Proposition::LessOrEqual(signed_value.clone(), positive_42.clone()),
    ]);
    assert_eq!(
        exact_integer_multiply_obligation(i8_type, signed_value.clone(), signed_three, &[],),
        expected.clone()
    );
    assert_eq!(
        exact_integer_multiply_obligation(i8_type, signed_value, negative_three, &[],),
        expected
    );

    let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).expect("-1i8");
    let minimum_plus_one =
        ScalarTerm::integer(i8_type, IntegerValue::Signed(-127)).expect("-127i8");
    let signed_value = ScalarTerm::value(
        ValueId::new(4).expect("value"),
        ScalarType::Integer(i8_type),
    );
    assert_eq!(
        exact_integer_multiply_obligation(i8_type, signed_value.clone(), negative_one, &[],),
        Proposition::LessOrEqual(minimum_plus_one, signed_value)
    );
}

#[test]
fn exact_multiply_chain_reconstructs_cumulative_parameter_bounds() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let u8_value = |id| {
        ScalarTerm::value(
            ValueId::new(id).expect("value"),
            ScalarType::Integer(u8_type),
        )
    };
    let root = u8_value(1);
    let first = u8_value(2);
    let second = u8_value(3);
    let two = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(2)).expect("2u8");
    let three = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(3)).expect("3u8");
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_multiply(u8_type, root.clone(), two.clone()).expect("root * 2"),
    );
    let second_definition = Proposition::Equal(
        second.clone(),
        ScalarTerm::exact_integer_multiply(u8_type, first, three).expect("first * 3"),
    );
    let axioms = vec![first_definition, second_definition];
    let parameters = BTreeSet::from([ValueId::new(1).expect("root")]);
    let twenty_one = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(21)).expect("21u8");
    assert_eq!(
        exact_integer_multiply_obligation_with_definitions(
            u8_type,
            second,
            two,
            &axioms,
            axioms.len(),
            &parameters,
        ),
        Proposition::LessOrEqual(root, twenty_one)
    );

    let reversed_factor = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(3)).expect("3u8");
    let direct_boundary = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(85)).expect("85u8");
    let reversed_left = axioms
        .last()
        .and_then(|axiom| match axiom {
            Proposition::Equal(left, _) => Some(left.clone()),
            _ => None,
        })
        .expect("second result");
    assert_eq!(
        exact_integer_multiply_obligation_with_definitions(
            u8_type,
            reversed_factor,
            reversed_left.clone(),
            &axioms,
            axioms.len(),
            &parameters,
        ),
        Proposition::LessOrEqual(reversed_left, direct_boundary),
        "a reversed outer factor does not gain chain-definition authority"
    );

    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_root = ScalarTerm::value(
        ValueId::new(4).expect("signed root"),
        ScalarType::Integer(i8_type),
    );
    let signed_first = ScalarTerm::value(
        ValueId::new(5).expect("signed first"),
        ScalarType::Integer(i8_type),
    );
    let signed_two = ScalarTerm::integer(i8_type, IntegerValue::Signed(2)).expect("2i8");
    let signed_three = ScalarTerm::integer(i8_type, IntegerValue::Signed(3)).expect("3i8");
    let signed_axioms = vec![Proposition::Equal(
        signed_first.clone(),
        ScalarTerm::exact_integer_multiply(i8_type, signed_root.clone(), signed_two)
            .expect("signed root * 2"),
    )];
    let signed_parameters = BTreeSet::from([ValueId::new(4).expect("signed root")]);
    let negative_twenty_one =
        ScalarTerm::integer(i8_type, IntegerValue::Signed(-21)).expect("-21i8");
    let positive_twenty_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(21)).expect("21i8");
    assert_eq!(
        exact_integer_multiply_obligation_with_definitions(
            i8_type,
            signed_first,
            signed_three,
            &signed_axioms,
            signed_axioms.len(),
            &signed_parameters,
        ),
        canonical_conjunction(vec![
            Proposition::LessOrEqual(negative_twenty_one, signed_root.clone()),
            Proposition::LessOrEqual(signed_root, positive_twenty_one),
        ])
    );
}

#[test]
fn signed_multiply_chains_reverse_preimages_and_preserve_zero_and_minimum() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 type");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 type");
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 type");
    let root_id = ValueId::new(1601).expect("signed-product root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(i8_type));
    let inner = ScalarTerm::value(
        ValueId::new(1602).expect("signed-product inner"),
        ScalarType::Integer(i8_type),
    );
    let definitions = vec![Proposition::Equal(
        inner.clone(),
        ScalarTerm::exact_integer_multiply(
            i8_type,
            root.clone(),
            ScalarTerm::integer(i8_type, IntegerValue::Signed(-2)).expect("-2i8"),
        )
        .expect("root * -2"),
    )];
    let expected = canonical_conjunction(vec![
        Proposition::LessOrEqual(
            ScalarTerm::integer(i8_type, IntegerValue::Signed(-21)).expect("-21i8"),
            root.clone(),
        ),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(i8_type, IntegerValue::Signed(21)).expect("21i8"),
        ),
    ]);
    assert_eq!(
        exact_integer_signed_multiply_chain_obligation(
            i8_type,
            inner.clone(),
            IntegerValue::Signed(3),
            &definitions,
            definitions.len(),
            &BTreeSet::from([root_id]),
        ),
        Some(expected.clone()),
        "a negative cumulative product reverses the carrier preimage",
    );
    assert_eq!(
        exact_integer_signed_multiply_chain_obligation(
            i8_type,
            inner.clone(),
            IntegerValue::Signed(-3),
            &definitions,
            definitions.len(),
            &BTreeSet::from([root_id]),
        ),
        Some(expected),
        "two negative factors restore the positive carrier preimage",
    );
    assert_eq!(
        exact_integer_signed_multiply_chain_obligation(
            i8_type,
            inner,
            IntegerValue::Signed(0),
            &definitions,
            definitions.len(),
            &BTreeSet::from([root_id]),
        ),
        Some(Proposition::Truth),
        "zero decides only the current prefix",
    );

    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64 type");
    let minimum_root_id = ValueId::new(1607).expect("minimum-factor root");
    let minimum_root = ScalarTerm::value(minimum_root_id, ScalarType::Integer(i64_type));
    let minimum_product = ScalarTerm::value(
        ValueId::new(1608).expect("minimum-factor product"),
        ScalarType::Integer(i64_type),
    );
    let minimum_definitions = vec![Proposition::Equal(
        minimum_product.clone(),
        ScalarTerm::exact_integer_multiply(
            i64_type,
            minimum_root.clone(),
            ScalarTerm::integer(i64_type, IntegerValue::Signed(i64::MIN.into())).expect("MIN i64"),
        )
        .expect("root * MIN"),
    )];
    assert_eq!(
        exact_integer_signed_multiply_chain_obligation(
            i64_type,
            minimum_product,
            IntegerValue::Signed(1),
            &minimum_definitions,
            minimum_definitions.len(),
            &BTreeSet::from([minimum_root_id]),
        ),
        Some(exact_integer_source_interval_obligation(
            i64_type,
            minimum_root,
            0,
            1,
        )),
        "signed MIN is accumulated by magnitude without host negation",
    );

    let wide_root_id = ValueId::new(1603).expect("wide root");
    let wide_root = ScalarTerm::value(wide_root_id, ScalarType::Integer(i16_type));
    let wide_product = ScalarTerm::value(
        ValueId::new(1604).expect("wide product"),
        ScalarType::Integer(i16_type),
    );
    let wide_definitions = vec![Proposition::Equal(
        wide_product.clone(),
        ScalarTerm::exact_integer_multiply(
            i16_type,
            wide_root.clone(),
            ScalarTerm::integer(i16_type, IntegerValue::Signed(-512)).expect("-512i16"),
        )
        .expect("wide root * -512"),
    )];
    assert_eq!(
        exact_integer_signed_multiply_chain_cast_obligation(
            i16_type,
            i8_type,
            wide_product,
            &wide_definitions,
            &BTreeSet::from([wide_root_id]),
        ),
        Some(exact_integer_source_interval_obligation(
            i16_type, wide_root, 0, 0,
        )),
        "the negative pre-cast product reverses the target interval",
    );

    let unsigned_root_id = ValueId::new(1605).expect("unsigned root");
    let unsigned_root = ScalarTerm::value(unsigned_root_id, ScalarType::Integer(u16_type));
    let cast = ScalarTerm::value(
        ValueId::new(1606).expect("signed cast"),
        ScalarType::Integer(i8_type),
    );
    let cast_definitions = vec![Proposition::Equal(
        cast.clone(),
        ScalarTerm::integer_exact_cast(u16_type, i8_type, unsigned_root.clone())
            .expect("u16 to i8 cast"),
    )];
    assert_eq!(
        exact_integer_cast_then_signed_multiply_chain_obligation(
            i8_type,
            cast,
            IntegerValue::Signed(-2),
            &cast_definitions,
            cast_definitions.len(),
            &BTreeSet::from([unsigned_root_id]),
        ),
        Some(exact_integer_source_interval_obligation(
            u16_type,
            unsigned_root,
            0,
            64,
        )),
        "post-cast negative multiplication intersects its reversed preimage with the source",
    );
}

#[test]
fn signed_affine_three_placement_replays_negative_coefficients_without_importing_prefix_proofs() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 type");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 type");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 type");
    let root_id = ValueId::new(1661).expect("signed-affine root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(i8_type));
    let offset = ScalarTerm::value(
        ValueId::new(1662).expect("signed-affine offset"),
        ScalarType::Integer(i8_type),
    );
    let negative = ScalarTerm::value(
        ValueId::new(1663).expect("signed-affine negative"),
        ScalarType::Integer(i8_type),
    );
    let definitions = vec![
        Proposition::Equal(
            offset.clone(),
            ScalarTerm::exact_integer_add(
                i8_type,
                root.clone(),
                ScalarTerm::integer(i8_type, IntegerValue::Signed(3)).expect("3i8"),
            )
            .expect("root + 3"),
        ),
        Proposition::Equal(
            negative.clone(),
            ScalarTerm::exact_integer_multiply(
                i8_type,
                offset,
                ScalarTerm::integer(i8_type, IntegerValue::Signed(-2)).expect("-2i8"),
            )
            .expect("offset * -2"),
        ),
    ];
    let parameters = BTreeSet::from([root_id]);
    assert_eq!(
        exact_integer_signed_affine_chain_obligation(
            i8_type,
            negative.clone(),
            IntegerValue::Signed(1),
            ExactIntegerAffineOperation::Subtract,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Some(exact_integer_source_interval_obligation(
            i8_type,
            root.clone(),
            -67,
            60,
        )),
        "a negative coefficient reverses the complete offset preimage",
    );
    assert_eq!(
        exact_integer_signed_affine_chain_cast_obligation(
            i8_type,
            u8_type,
            negative,
            &definitions,
            &parameters,
        ),
        Some(exact_integer_source_interval_obligation(
            i8_type,
            root.clone(),
            -128,
            -3,
        )),
        "the pre-cast obligation replays the signed source form independently",
    );

    let wide_root_id = ValueId::new(1664).expect("post-cast root");
    let wide_root = ScalarTerm::value(wide_root_id, ScalarType::Integer(i16_type));
    let cast = ScalarTerm::value(
        ValueId::new(1665).expect("post-cast value"),
        ScalarType::Integer(i8_type),
    );
    let cast_offset = ScalarTerm::value(
        ValueId::new(1666).expect("post-cast offset"),
        ScalarType::Integer(i8_type),
    );
    let post_definitions = vec![
        Proposition::Equal(
            cast.clone(),
            ScalarTerm::integer_exact_cast(i16_type, i8_type, wide_root.clone())
                .expect("i16 to i8 cast"),
        ),
        Proposition::Equal(
            cast_offset.clone(),
            ScalarTerm::exact_integer_add(
                i8_type,
                cast,
                ScalarTerm::integer(i8_type, IntegerValue::Signed(3)).expect("3i8"),
            )
            .expect("cast + 3"),
        ),
    ];
    assert_eq!(
        exact_integer_cast_then_signed_affine_chain_obligation(
            i8_type,
            cast_offset,
            IntegerValue::Signed(-2),
            ExactIntegerAffineOperation::Multiply,
            &post_definitions,
            post_definitions.len(),
            &BTreeSet::from([wide_root_id]),
        ),
        Some(exact_integer_source_interval_obligation(
            i16_type, wide_root, -66, 61,
        )),
        "the post-cast prefix intersects the reversed target preimage with the source carrier",
    );

    let zero = ScalarTerm::value(
        ValueId::new(1667).expect("zero result"),
        ScalarType::Integer(i8_type),
    );
    let zero_definitions = definitions
        .iter()
        .cloned()
        .chain([Proposition::Equal(
            zero.clone(),
            ScalarTerm::exact_integer_multiply(
                i8_type,
                ScalarTerm::value(
                    ValueId::new(1663).expect("signed-affine negative"),
                    ScalarType::Integer(i8_type),
                ),
                ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).expect("0i8"),
            )
            .expect("negative * 0"),
        )])
        .collect::<Vec<_>>();
    assert_eq!(
        exact_integer_signed_affine_chain_obligation(
            i8_type,
            zero,
            IntegerValue::Signed(-128),
            ExactIntegerAffineOperation::Subtract,
            &zero_definitions,
            zero_definitions.len(),
            &parameters,
        ),
        Some(Proposition::Falsehood),
        "a constant outside the carrier is mathematical falsehood, not checked failure",
    );

    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64 type");
    let minimum_root_id = ValueId::new(1668).expect("MIN affine root");
    let minimum_root = ScalarTerm::value(minimum_root_id, ScalarType::Integer(i64_type));
    let minimum_offset = ScalarTerm::value(
        ValueId::new(1669).expect("MIN affine offset"),
        ScalarType::Integer(i64_type),
    );
    let minimum_definitions = vec![Proposition::Equal(
        minimum_offset.clone(),
        ScalarTerm::exact_integer_add(
            i64_type,
            minimum_root.clone(),
            ScalarTerm::integer(i64_type, IntegerValue::Signed(0)).expect("0i64"),
        )
        .expect("root + 0"),
    )];
    assert_eq!(
        exact_integer_signed_affine_chain_obligation(
            i64_type,
            minimum_offset,
            IntegerValue::Signed(i64::MIN.into()),
            ExactIntegerAffineOperation::Multiply,
            &minimum_definitions,
            minimum_definitions.len(),
            &BTreeSet::from([minimum_root_id]),
        ),
        Some(exact_integer_source_interval_obligation(
            i64_type,
            minimum_root,
            0,
            1,
        )),
        "MIN is handled by magnitude without host negation",
    );

    let overflow_first = ScalarTerm::value(
        ValueId::new(1670).expect("overflow first"),
        ScalarType::Integer(i64_type),
    );
    let overflow_second = ScalarTerm::value(
        ValueId::new(1671).expect("overflow second"),
        ScalarType::Integer(i64_type),
    );
    let overflow_third = ScalarTerm::value(
        ValueId::new(1672).expect("overflow third"),
        ScalarType::Integer(i64_type),
    );
    let overflow_definitions = vec![
        Proposition::Equal(
            overflow_first.clone(),
            ScalarTerm::exact_integer_add(
                i64_type,
                ScalarTerm::value(minimum_root_id, ScalarType::Integer(i64_type)),
                ScalarTerm::integer(i64_type, IntegerValue::Signed(1)).expect("1i64"),
            )
            .expect("root + 1"),
        ),
        Proposition::Equal(
            overflow_second.clone(),
            ScalarTerm::exact_integer_multiply(
                i64_type,
                overflow_first,
                ScalarTerm::integer(i64_type, IntegerValue::Signed(i64::MIN.into()))
                    .expect("MIN i64"),
            )
            .expect("offset * MIN"),
        ),
        Proposition::Equal(
            overflow_third.clone(),
            ScalarTerm::exact_integer_multiply(
                i64_type,
                overflow_second,
                ScalarTerm::integer(i64_type, IntegerValue::Signed(i64::MIN.into()))
                    .expect("MIN i64"),
            )
            .expect("MIN product * MIN"),
        ),
    ];
    assert_eq!(
        exact_integer_signed_affine_chain_obligation(
            i64_type,
            overflow_third,
            IntegerValue::Signed(4),
            ExactIntegerAffineOperation::Multiply,
            &overflow_definitions,
            overflow_definitions.len(),
            &BTreeSet::from([minimum_root_id]),
        ),
        None,
        "checked coefficient or offset overflow admits no family",
    );
    assert_eq!(
        exact_integer_signed_affine_chain_obligation(
            i8_type,
            ScalarTerm::value(
                ValueId::new(1673).expect("stale value"),
                ScalarType::Integer(i8_type),
            ),
            IntegerValue::Signed(-2),
            ExactIntegerAffineOperation::Multiply,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        None,
        "a stale definition cannot borrow the valid chain's evidence",
    );
}
