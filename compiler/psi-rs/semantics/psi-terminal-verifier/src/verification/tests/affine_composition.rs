use super::super::*;
use psi_core::{IntegerType, ScalarType, ValueId};

#[test]
fn mixed_affine_chain_reconstructs_every_prefix_and_constant_collapse() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let root_id = ValueId::new(601).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(u8_type));
    let added = ScalarTerm::value(
        ValueId::new(602).expect("added"),
        ScalarType::Integer(u8_type),
    );
    let multiplied = ScalarTerm::value(
        ValueId::new(603).expect("multiplied"),
        ScalarType::Integer(u8_type),
    );
    let three = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(3)).expect("3u8");
    let two = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(2)).expect("2u8");
    let definitions = vec![
        Proposition::Equal(
            added.clone(),
            ScalarTerm::exact_integer_add(u8_type, root.clone(), three).expect("root + 3"),
        ),
        Proposition::Equal(
            multiplied.clone(),
            ScalarTerm::exact_integer_multiply(u8_type, added.clone(), two).expect("added * 2"),
        ),
    ];
    let parameters = BTreeSet::from([root_id]);
    assert_eq!(
        exact_integer_affine_chain_obligation(
            u8_type,
            added.clone(),
            IntegerValue::Unsigned(2),
            ExactIntegerAffineOperation::Multiply,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Some(Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(124)).expect("124u8"),
        )),
        "the multiply prefix independently reconstructs 2*p + 6"
    );
    assert_eq!(
        exact_integer_affine_chain_obligation(
            u8_type,
            multiplied.clone(),
            IntegerValue::Unsigned(1),
            ExactIntegerAffineOperation::Subtract,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Some(Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(125)).expect("125u8"),
        )),
        "the subtract prefix independently reconstructs 2*p + 5"
    );
    assert_eq!(
        exact_integer_affine_chain_obligation(
            u8_type,
            added.clone(),
            IntegerValue::Unsigned(0),
            ExactIntegerAffineOperation::Multiply,
            &definitions,
            1,
            &parameters,
        ),
        Some(Proposition::Truth),
        "a zero factor makes only the current prefix constant while the add proof remains"
    );
    assert_eq!(
        exact_integer_affine_chain_obligation(
            u8_type,
            multiplied,
            IntegerValue::Unsigned(1),
            ExactIntegerAffineOperation::Subtract,
            &[definitions[1].clone(), definitions[0].clone()],
            definitions.len(),
            &parameters,
        ),
        None,
        "reordered definitions cannot authorize the affine walk"
    );

    let signed_root_id = ValueId::new(611).expect("signed root");
    let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(i8_type));
    assert_eq!(
        exact_integer_affine_interval_obligation(
            i8_type,
            signed_root.clone(),
            2,
            IntegerOffset::Nonnegative(7),
        ),
        canonical_conjunction(vec![
            Proposition::LessOrEqual(
                ScalarTerm::integer(i8_type, IntegerValue::Signed(-67)).expect("-67i8"),
                signed_root.clone(),
            ),
            Proposition::LessOrEqual(
                signed_root,
                ScalarTerm::integer(i8_type, IntegerValue::Signed(60)).expect("60i8"),
            ),
        ]),
        "signed affine inversion uses ceiling for the lower and floor for the upper bound"
    );
}

#[test]
fn exact_multiply_chain_handles_zero_one_and_accumulator_overflow() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let value = |id| {
        ScalarTerm::value(
            ValueId::new(id).expect("value"),
            ScalarType::Integer(u8_type),
        )
    };
    let root = value(1);
    let first = value(2);
    let second = value(3);
    let one = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).expect("1u8");
    let two = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(2)).expect("2u8");
    let zero = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(0)).expect("0u8");
    let seven = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(7)).expect("7u8");
    let identity_axioms = vec![Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_multiply(u8_type, root.clone(), one.clone()).expect("root * 1"),
    )];
    assert_eq!(
        exact_integer_multiply_obligation_with_definitions(
            u8_type,
            first.clone(),
            one,
            &identity_axioms,
            identity_axioms.len(),
            &BTreeSet::from([ValueId::new(1).expect("root")]),
        ),
        Proposition::Truth
    );
    let axioms = vec![
        Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_multiply(u8_type, root, two).expect("root * 2"),
        ),
        Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_multiply(u8_type, first, zero).expect("first * 0"),
        ),
    ];
    assert_eq!(
        exact_integer_multiply_obligation_with_definitions(
            u8_type,
            second,
            seven,
            &axioms,
            axioms.len(),
            &BTreeSet::from([ValueId::new(1).expect("root")]),
        ),
        Proposition::Truth
    );

    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let value = |id| {
        ScalarTerm::value(
            ValueId::new(id).expect("value"),
            ScalarType::Integer(u64_type),
        )
    };
    let root = value(10);
    let first = value(11);
    let second = value(12);
    let maximum =
        ScalarTerm::integer(u64_type, IntegerValue::Unsigned(u64::MAX.into())).expect("u64::MAX");
    let axioms = vec![
        Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_multiply(u64_type, root, maximum.clone())
                .expect("root * MAX"),
        ),
        Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_multiply(u64_type, first, maximum.clone())
                .expect("first * MAX"),
        ),
    ];
    assert_eq!(
        exact_integer_multiply_obligation_with_definitions(
            u64_type,
            second,
            maximum,
            &axioms,
            axioms.len(),
            &BTreeSet::from([ValueId::new(10).expect("root")]),
        ),
        Proposition::Falsehood
    );
}

#[test]
fn exact_multiply_chain_after_partial_cast_reconstructs_every_scaled_source_interval() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let root_id = ValueId::new(201).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
    let cast = ScalarTerm::value(
        ValueId::new(202).expect("cast"),
        ScalarType::Integer(target_type),
    );
    let first = ScalarTerm::value(
        ValueId::new(203).expect("first product"),
        ScalarType::Integer(target_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(204).expect("second product"),
        ScalarType::Integer(target_type),
    );
    let cast_definition = Proposition::Equal(
        cast.clone(),
        ScalarTerm::integer_exact_cast(source_type, target_type, root.clone())
            .expect("u16 to u8 exact cast"),
    );
    let two = ScalarTerm::integer(target_type, IntegerValue::Unsigned(2)).expect("2u8");
    let three = ScalarTerm::integer(target_type, IntegerValue::Unsigned(3)).expect("3u8");
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_multiply(target_type, cast.clone(), two.clone())
            .expect("cast * 2u8"),
    );
    let second_definition = Proposition::Equal(
        second.clone(),
        ScalarTerm::exact_integer_multiply(target_type, first.clone(), three.clone())
            .expect("(cast * 2u8) * 3u8"),
    );
    let parameters = BTreeSet::from([root_id]);
    assert_eq!(
        exact_integer_multiply_obligation_with_definitions(
            target_type,
            cast.clone(),
            two.clone(),
            std::slice::from_ref(&cast_definition),
            1,
            &parameters,
        ),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(127)).expect("127u16"),
        )
    );
    assert_eq!(
        exact_integer_multiply_obligation_with_definitions(
            target_type,
            first.clone(),
            three,
            &[cast_definition.clone(), first_definition.clone()],
            2,
            &parameters,
        ),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(42)).expect("42u16"),
        )
    );
    assert_eq!(
        exact_integer_multiply_obligation_with_definitions(
            target_type,
            second,
            ScalarTerm::integer(target_type, IntegerValue::Unsigned(0)).expect("0u8"),
            &[
                cast_definition.clone(),
                first_definition.clone(),
                second_definition,
            ],
            3,
            &parameters,
        ),
        Proposition::Truth,
        "a zero factor makes only the current prefix total"
    );

    let signed_source = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let signed_target = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_root_id = ValueId::new(211).expect("signed root");
    let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(signed_source));
    let signed_cast = ScalarTerm::value(
        ValueId::new(212).expect("signed cast"),
        ScalarType::Integer(signed_target),
    );
    let signed_first = ScalarTerm::value(
        ValueId::new(213).expect("signed first product"),
        ScalarType::Integer(signed_target),
    );
    let signed_cast_definition = Proposition::Equal(
        signed_cast.clone(),
        ScalarTerm::integer_exact_cast(signed_source, signed_target, signed_root.clone())
            .expect("i16 to i8 exact cast"),
    );
    let signed_first_definition = Proposition::Equal(
        signed_first.clone(),
        ScalarTerm::exact_integer_multiply(
            signed_target,
            signed_cast,
            ScalarTerm::integer(signed_target, IntegerValue::Signed(2)).expect("2i8"),
        )
        .expect("signed cast * 2i8"),
    );
    assert_eq!(
        exact_integer_multiply_obligation_with_definitions(
            signed_target,
            signed_first,
            ScalarTerm::integer(signed_target, IntegerValue::Signed(3)).expect("3i8"),
            &[signed_cast_definition, signed_first_definition],
            2,
            &BTreeSet::from([signed_root_id]),
        ),
        canonical_conjunction(vec![
            Proposition::LessOrEqual(
                ScalarTerm::integer(signed_source, IntegerValue::Signed(-21)).expect("-21i16"),
                signed_root.clone(),
            ),
            Proposition::LessOrEqual(
                signed_root,
                ScalarTerm::integer(signed_source, IntegerValue::Signed(21)).expect("21i16"),
            ),
        ])
    );

    let cross_source = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let cross_target = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let cross_root_id = ValueId::new(221).expect("cross root");
    let cross_root = ScalarTerm::value(cross_root_id, ScalarType::Integer(cross_source));
    let cross_cast = ScalarTerm::value(
        ValueId::new(222).expect("cross cast"),
        ScalarType::Integer(cross_target),
    );
    let cross_definition = Proposition::Equal(
        cross_cast.clone(),
        ScalarTerm::integer_exact_cast(cross_source, cross_target, cross_root.clone())
            .expect("i8 to u8 exact cast"),
    );
    assert_eq!(
        exact_integer_multiply_obligation_with_definitions(
            cross_target,
            cross_cast,
            ScalarTerm::integer(cross_target, IntegerValue::Unsigned(2)).expect("2u8"),
            std::slice::from_ref(&cross_definition),
            1,
            &BTreeSet::from([cross_root_id]),
        ),
        Proposition::LessOrEqual(
            ScalarTerm::integer(cross_source, IntegerValue::Signed(0)).expect("0i8"),
            cross_root,
        )
    );

    let reversed_cast_definition = match cast_definition.clone() {
        Proposition::Equal(left, right) => Proposition::Equal(right, left),
        _ => unreachable!("cast definition is an equality"),
    };
    assert_ne!(
        exact_integer_multiply_obligation_with_definitions(
            target_type,
            cast.clone(),
            two.clone(),
            std::slice::from_ref(&reversed_cast_definition),
            1,
            &parameters,
        ),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(127)).expect("127u16"),
        )
    );
    assert_ne!(
        exact_integer_multiply_obligation_with_definitions(
            target_type,
            two,
            cast,
            std::slice::from_ref(&cast_definition),
            1,
            &parameters,
        ),
        Proposition::LessOrEqual(
            root,
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(127)).expect("127u16"),
        ),
        "literal-left multiplication does not gain definition authority"
    );

    let wide_source = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let wide_target = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let wide_root_id = ValueId::new(231).expect("wide root");
    let wide_root = ScalarTerm::value(wide_root_id, ScalarType::Integer(wide_source));
    let wide_cast = ScalarTerm::value(
        ValueId::new(232).expect("wide cast"),
        ScalarType::Integer(wide_target),
    );
    let wide_first = ScalarTerm::value(
        ValueId::new(233).expect("wide first product"),
        ScalarType::Integer(wide_target),
    );
    let wide_second = ScalarTerm::value(
        ValueId::new(234).expect("wide second product"),
        ScalarType::Integer(wide_target),
    );
    let maximum = ScalarTerm::integer(wide_target, IntegerValue::Unsigned(u128::from(u64::MAX)))
        .expect("u64::MAX");
    let wide_definitions = [
        Proposition::Equal(
            wide_cast.clone(),
            ScalarTerm::integer_exact_cast(wide_source, wide_target, wide_root)
                .expect("i64 to u64 exact cast"),
        ),
        Proposition::Equal(
            wide_first.clone(),
            ScalarTerm::exact_integer_multiply(wide_target, wide_cast, maximum.clone())
                .expect("wide cast * MAX"),
        ),
        Proposition::Equal(
            wide_second.clone(),
            ScalarTerm::exact_integer_multiply(wide_target, wide_first, maximum.clone())
                .expect("wide first * MAX"),
        ),
    ];
    assert_eq!(
        exact_integer_multiply_obligation_with_definitions(
            wide_target,
            wide_second,
            maximum,
            &wide_definitions,
            wide_definitions.len(),
            &BTreeSet::from([wide_root_id]),
        ),
        Proposition::Falsehood,
        "cumulative product overflow fails closed"
    );
}

#[test]
fn mixed_affine_chain_after_partial_cast_reconstructs_each_prefix_independently() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let root_id = ValueId::new(701).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
    let value = |id| {
        ScalarTerm::value(
            ValueId::new(id).expect("value"),
            ScalarType::Integer(target_type),
        )
    };
    let cast = value(702);
    let added = value(703);
    let multiplied = value(704);
    let definitions = vec![
        Proposition::Equal(
            cast.clone(),
            ScalarTerm::integer_exact_cast(source_type, target_type, root.clone())
                .expect("u16 to u8 exact cast"),
        ),
        Proposition::Equal(
            added.clone(),
            ScalarTerm::exact_integer_add(
                target_type,
                cast.clone(),
                ScalarTerm::integer(target_type, IntegerValue::Unsigned(3)).expect("3u8"),
            )
            .expect("cast + 3"),
        ),
        Proposition::Equal(
            multiplied.clone(),
            ScalarTerm::exact_integer_multiply(
                target_type,
                added.clone(),
                ScalarTerm::integer(target_type, IntegerValue::Unsigned(2)).expect("2u8"),
            )
            .expect("added * 2"),
        ),
    ];
    let parameters = BTreeSet::from([root_id]);
    assert_eq!(
        exact_integer_multiply_obligation_with_definitions(
            target_type,
            added.clone(),
            ScalarTerm::integer(target_type, IntegerValue::Unsigned(2)).expect("2u8"),
            &definitions[..2],
            2,
            &parameters,
        ),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(124)).expect("124u16"),
        ),
    );
    assert_eq!(
        exact_integer_subtract_obligation(
            target_type,
            multiplied,
            ScalarTerm::integer(target_type, IntegerValue::Unsigned(1)).expect("1u8"),
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(125)).expect("125u16"),
        ),
    );
    assert_eq!(
        exact_integer_cast_then_affine_chain_obligation(
            target_type,
            added,
            IntegerValue::Unsigned(0),
            ExactIntegerAffineOperation::Multiply,
            &definitions[..2],
            2,
            &parameters,
        ),
        Some(Proposition::Truth),
        "zero collapse discharges only the current arithmetic prefix",
    );
    assert_eq!(
        exact_integer_cast_then_affine_chain_obligation(
            target_type,
            value(705),
            IntegerValue::Unsigned(1),
            ExactIntegerAffineOperation::Subtract,
            &[definitions[1].clone(), definitions[0].clone()],
            2,
            &parameters,
        ),
        None,
        "reordered or stale definitions cannot authorize the walk",
    );
}

#[test]
fn affine_cast_affine_reconstructs_both_sides_without_importing_any_prefix_proof() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let root_id = ValueId::new(711).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
    let source_add = ScalarTerm::value(
        ValueId::new(712).expect("source add"),
        ScalarType::Integer(source_type),
    );
    let source_multiply = ScalarTerm::value(
        ValueId::new(713).expect("source multiply"),
        ScalarType::Integer(source_type),
    );
    let cast = ScalarTerm::value(
        ValueId::new(714).expect("cast"),
        ScalarType::Integer(target_type),
    );
    let target_subtract = ScalarTerm::value(
        ValueId::new(715).expect("target subtract"),
        ScalarType::Integer(target_type),
    );
    let definitions = vec![
        Proposition::Equal(
            source_add.clone(),
            ScalarTerm::exact_integer_add(
                source_type,
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(3)).expect("3u16"),
            )
            .expect("root + 3"),
        ),
        Proposition::Equal(
            source_multiply.clone(),
            ScalarTerm::exact_integer_multiply(
                source_type,
                source_add,
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(2)).expect("2u16"),
            )
            .expect("(root + 3) * 2"),
        ),
        Proposition::Equal(
            cast.clone(),
            ScalarTerm::integer_exact_cast(source_type, target_type, source_multiply)
                .expect("affine u16 to u8 cast"),
        ),
        Proposition::Equal(
            target_subtract.clone(),
            ScalarTerm::exact_integer_subtract(
                target_type,
                cast,
                ScalarTerm::integer(target_type, IntegerValue::Unsigned(1)).expect("1u8"),
            )
            .expect("cast - 1"),
        ),
    ];
    assert_eq!(
        exact_integer_affine_cast_affine_obligation(
            target_type,
            target_subtract.clone(),
            IntegerValue::Unsigned(2),
            ExactIntegerAffineOperation::Multiply,
            &definitions,
            definitions.len(),
            &BTreeSet::from([root_id]),
        ),
        Some(Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(61)).expect("61u16"),
        )),
    );
    assert_eq!(
        exact_integer_affine_cast_affine_obligation(
            target_type,
            target_subtract,
            IntegerValue::Unsigned(2),
            ExactIntegerAffineOperation::Multiply,
            &[Proposition::Truth],
            1,
            &BTreeSet::from([root_id]),
        ),
        None,
        "stale definitions cannot authorize either side of the cast sandwich",
    );

    let source_zero = ScalarTerm::value(
        ValueId::new(716).expect("source zero"),
        ScalarType::Integer(source_type),
    );
    let zero_cast = ScalarTerm::value(
        ValueId::new(717).expect("zero cast"),
        ScalarType::Integer(target_type),
    );
    let source_zero_definitions = vec![
        Proposition::Equal(
            source_zero.clone(),
            ScalarTerm::exact_integer_multiply(
                source_type,
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(0)).expect("0u16"),
            )
            .expect("root * 0"),
        ),
        Proposition::Equal(
            zero_cast.clone(),
            ScalarTerm::integer_exact_cast(source_type, target_type, source_zero)
                .expect("zero u16 to u8 cast"),
        ),
    ];
    assert_eq!(
        exact_integer_affine_cast_affine_obligation(
            target_type,
            zero_cast,
            IntegerValue::Unsigned(1),
            ExactIntegerAffineOperation::Add,
            &source_zero_definitions,
            source_zero_definitions.len(),
            &BTreeSet::from([root_id]),
        ),
        Some(Proposition::Truth),
        "a source-side zero decides only the current post-cast obligation",
    );

    let post_zero = ScalarTerm::value(
        ValueId::new(718).expect("post zero"),
        ScalarType::Integer(target_type),
    );
    let mut post_zero_definitions = definitions[..3].to_vec();
    post_zero_definitions.push(Proposition::Equal(
        post_zero.clone(),
        ScalarTerm::exact_integer_multiply(
            target_type,
            ScalarTerm::value(
                ValueId::new(714).expect("cast"),
                ScalarType::Integer(target_type),
            ),
            ScalarTerm::integer(target_type, IntegerValue::Unsigned(0)).expect("0u8"),
        )
        .expect("cast * 0"),
    ));
    assert_eq!(
        exact_integer_affine_cast_affine_obligation(
            target_type,
            post_zero,
            IntegerValue::Unsigned(255),
            ExactIntegerAffineOperation::Add,
            &post_zero_definitions,
            post_zero_definitions.len(),
            &BTreeSet::from([root_id]),
        ),
        Some(Proposition::Truth),
        "a target-side zero decides only the current obligation after full source validation",
    );
}

#[test]
fn signed_affine_cast_affine_reverses_each_side_and_keeps_zero_local() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 type");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 type");
    let root_id = ValueId::new(1761).expect("source-negative root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(i16_type));
    let source_add = ScalarTerm::value(
        ValueId::new(1762).expect("source-negative add"),
        ScalarType::Integer(i16_type),
    );
    let source_negative = ScalarTerm::value(
        ValueId::new(1763).expect("source-negative product"),
        ScalarType::Integer(i16_type),
    );
    let cast = ScalarTerm::value(
        ValueId::new(1764).expect("source-negative cast"),
        ScalarType::Integer(i8_type),
    );
    let target_add = ScalarTerm::value(
        ValueId::new(1765).expect("source-negative target add"),
        ScalarType::Integer(i8_type),
    );
    let source_negative_definitions = vec![
        Proposition::Equal(
            source_add.clone(),
            ScalarTerm::exact_integer_add(
                i16_type,
                root.clone(),
                ScalarTerm::integer(i16_type, IntegerValue::Signed(3)).expect("3i16"),
            )
            .expect("root + 3"),
        ),
        Proposition::Equal(
            source_negative.clone(),
            ScalarTerm::exact_integer_multiply(
                i16_type,
                source_add,
                ScalarTerm::integer(i16_type, IntegerValue::Signed(-2)).expect("-2i16"),
            )
            .expect("source add * -2"),
        ),
        Proposition::Equal(
            cast.clone(),
            ScalarTerm::integer_exact_cast(i16_type, i8_type, source_negative)
                .expect("signed affine i16 to i8 cast"),
        ),
        Proposition::Equal(
            target_add.clone(),
            ScalarTerm::exact_integer_add(
                i8_type,
                cast.clone(),
                ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("cast + 1"),
        ),
    ];
    let source_negative_parameters = BTreeSet::from([root_id]);
    assert_eq!(
        exact_integer_signed_affine_cast_affine_obligation(
            i8_type,
            target_add.clone(),
            IntegerValue::Signed(2),
            ExactIntegerAffineOperation::Multiply,
            &source_negative_definitions,
            source_negative_definitions.len(),
            &source_negative_parameters,
        ),
        Some(exact_integer_source_interval_obligation(
            i16_type,
            root.clone(),
            -34,
            29,
        )),
        "a source-negative form reverses the target interval after cast intersection",
    );

    let target_negative_root_id = ValueId::new(1771).expect("target-negative root");
    let target_negative_root =
        ScalarTerm::value(target_negative_root_id, ScalarType::Integer(i16_type));
    let source_positive_add = ScalarTerm::value(
        ValueId::new(1772).expect("source-positive add"),
        ScalarType::Integer(i16_type),
    );
    let source_positive = ScalarTerm::value(
        ValueId::new(1773).expect("source-positive product"),
        ScalarType::Integer(i16_type),
    );
    let target_negative_cast = ScalarTerm::value(
        ValueId::new(1774).expect("target-negative cast"),
        ScalarType::Integer(i8_type),
    );
    let target_negative_add = ScalarTerm::value(
        ValueId::new(1775).expect("target-negative add"),
        ScalarType::Integer(i8_type),
    );
    let target_negative_definitions = vec![
        Proposition::Equal(
            source_positive_add.clone(),
            ScalarTerm::exact_integer_add(
                i16_type,
                target_negative_root.clone(),
                ScalarTerm::integer(i16_type, IntegerValue::Signed(3)).expect("3i16"),
            )
            .expect("root + 3"),
        ),
        Proposition::Equal(
            source_positive.clone(),
            ScalarTerm::exact_integer_multiply(
                i16_type,
                source_positive_add,
                ScalarTerm::integer(i16_type, IntegerValue::Signed(2)).expect("2i16"),
            )
            .expect("source add * 2"),
        ),
        Proposition::Equal(
            target_negative_cast.clone(),
            ScalarTerm::integer_exact_cast(i16_type, i8_type, source_positive)
                .expect("positive affine i16 to i8 cast"),
        ),
        Proposition::Equal(
            target_negative_add.clone(),
            ScalarTerm::exact_integer_add(
                i8_type,
                target_negative_cast,
                ScalarTerm::integer(i8_type, IntegerValue::Signed(3)).expect("3i8"),
            )
            .expect("cast + 3"),
        ),
    ];
    assert_eq!(
        exact_integer_signed_affine_cast_affine_obligation(
            i8_type,
            target_negative_add,
            IntegerValue::Signed(-2),
            ExactIntegerAffineOperation::Multiply,
            &target_negative_definitions,
            target_negative_definitions.len(),
            &BTreeSet::from([target_negative_root_id]),
        ),
        Some(exact_integer_source_interval_obligation(
            i16_type,
            target_negative_root,
            -36,
            27,
        )),
        "a target-negative form reverses before the source-positive inverse",
    );

    let target_zero = ScalarTerm::value(
        ValueId::new(1766).expect("target zero"),
        ScalarType::Integer(i8_type),
    );
    let mut zero_definitions = source_negative_definitions.clone();
    zero_definitions.push(Proposition::Equal(
        target_zero.clone(),
        ScalarTerm::exact_integer_multiply(
            i8_type,
            cast,
            ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).expect("0i8"),
        )
        .expect("cast * 0"),
    ));
    assert_eq!(
        exact_integer_signed_affine_cast_affine_obligation(
            i8_type,
            target_zero.clone(),
            IntegerValue::Signed(127),
            ExactIntegerAffineOperation::Add,
            &zero_definitions,
            zero_definitions.len(),
            &source_negative_parameters,
        ),
        Some(Proposition::Truth),
        "a target zero decides only its current representable constant",
    );
    assert_eq!(
        exact_integer_signed_affine_cast_affine_obligation(
            i8_type,
            target_zero,
            IntegerValue::Signed(-128),
            ExactIntegerAffineOperation::Subtract,
            &zero_definitions,
            zero_definitions.len(),
            &source_negative_parameters,
        ),
        Some(Proposition::Falsehood),
        "an unrepresentable target constant is mathematical falsehood",
    );
    assert_eq!(
        exact_integer_signed_affine_cast_affine_obligation(
            i8_type,
            target_add,
            IntegerValue::Signed(2),
            ExactIntegerAffineOperation::Multiply,
            &[Proposition::Truth],
            1,
            &source_negative_parameters,
        ),
        None,
        "stale definitions cannot authorize either sign-reversing side",
    );

    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64 type");
    let minimum_root_id = ValueId::new(1781).expect("MIN sandwich root");
    let minimum_root = ScalarTerm::value(minimum_root_id, ScalarType::Integer(i64_type));
    let minimum_offset = ScalarTerm::value(
        ValueId::new(1782).expect("MIN sandwich offset"),
        ScalarType::Integer(i64_type),
    );
    let minimum_product = ScalarTerm::value(
        ValueId::new(1783).expect("MIN sandwich product"),
        ScalarType::Integer(i64_type),
    );
    let minimum_cast = ScalarTerm::value(
        ValueId::new(1784).expect("MIN sandwich cast"),
        ScalarType::Integer(i8_type),
    );
    let minimum_definitions = vec![
        Proposition::Equal(
            minimum_offset.clone(),
            ScalarTerm::exact_integer_add(
                i64_type,
                minimum_root.clone(),
                ScalarTerm::integer(i64_type, IntegerValue::Signed(0)).expect("0i64"),
            )
            .expect("MIN root + 0"),
        ),
        Proposition::Equal(
            minimum_product.clone(),
            ScalarTerm::exact_integer_multiply(
                i64_type,
                minimum_offset,
                ScalarTerm::integer(i64_type, IntegerValue::Signed(i64::MIN.into()))
                    .expect("MIN i64"),
            )
            .expect("root * MIN"),
        ),
        Proposition::Equal(
            minimum_cast.clone(),
            ScalarTerm::integer_exact_cast(i64_type, i8_type, minimum_product)
                .expect("MIN product to i8 cast"),
        ),
    ];
    assert_eq!(
        exact_integer_signed_affine_cast_affine_obligation(
            i8_type,
            minimum_cast,
            IntegerValue::Signed(0),
            ExactIntegerAffineOperation::Add,
            &minimum_definitions,
            minimum_definitions.len(),
            &BTreeSet::from([minimum_root_id]),
        ),
        Some(exact_integer_source_interval_obligation(
            i64_type,
            minimum_root,
            0,
            0,
        )),
        "MIN is accumulated by magnitude without host negation",
    );

    let overflow_first = ScalarTerm::value(
        ValueId::new(1785).expect("overflow first"),
        ScalarType::Integer(i64_type),
    );
    let overflow_second = ScalarTerm::value(
        ValueId::new(1786).expect("overflow second"),
        ScalarType::Integer(i64_type),
    );
    let overflow_third = ScalarTerm::value(
        ValueId::new(1787).expect("overflow third"),
        ScalarType::Integer(i64_type),
    );
    let overflow_cast = ScalarTerm::value(
        ValueId::new(1788).expect("overflow cast"),
        ScalarType::Integer(i8_type),
    );
    let mut overflow_definitions = minimum_definitions[..1].to_vec();
    overflow_definitions.extend([
        Proposition::Equal(
            overflow_first.clone(),
            ScalarTerm::exact_integer_multiply(
                i64_type,
                ScalarTerm::value(
                    ValueId::new(1782).expect("MIN sandwich offset"),
                    ScalarType::Integer(i64_type),
                ),
                ScalarTerm::integer(i64_type, IntegerValue::Signed(i64::MIN.into()))
                    .expect("MIN i64"),
            )
            .expect("offset * MIN"),
        ),
        Proposition::Equal(
            overflow_second.clone(),
            ScalarTerm::exact_integer_multiply(
                i64_type,
                overflow_first,
                ScalarTerm::integer(i64_type, IntegerValue::Signed(i64::MIN.into()))
                    .expect("MIN i64"),
            )
            .expect("first * MIN"),
        ),
        Proposition::Equal(
            overflow_third.clone(),
            ScalarTerm::exact_integer_multiply(
                i64_type,
                overflow_second,
                ScalarTerm::integer(i64_type, IntegerValue::Signed(4)).expect("4i64"),
            )
            .expect("second * 4"),
        ),
        Proposition::Equal(
            overflow_cast.clone(),
            ScalarTerm::integer_exact_cast(i64_type, i8_type, overflow_third)
                .expect("overflow product to i8 cast"),
        ),
    ]);
    assert_eq!(
        exact_integer_signed_affine_cast_affine_obligation(
            i8_type,
            overflow_cast,
            IntegerValue::Signed(1),
            ExactIntegerAffineOperation::Add,
            &overflow_definitions,
            overflow_definitions.len(),
            &BTreeSet::from([minimum_root_id]),
        ),
        None,
        "checked coefficient overflow admits no sandwich family",
    );
}
