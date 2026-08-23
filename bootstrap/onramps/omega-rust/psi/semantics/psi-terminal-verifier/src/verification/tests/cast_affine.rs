use super::super::*;
use psi_core::{IntegerType, ScalarType, ValueId};

#[test]
fn reconstructs_widen_then_exact_narrow_roundtrip_as_self_proving() {
    let narrow_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let wide_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let input_id = ValueId::new(1).expect("input");
    let input = ScalarTerm::value(input_id, ScalarType::Integer(narrow_type));
    let widened = ScalarTerm::value(
        ValueId::new(2).expect("widened"),
        ScalarType::Integer(wide_type),
    );
    let definition = Proposition::Equal(
        widened.clone(),
        ScalarTerm::integer_widen(narrow_type, wide_type, input).expect("u8 to u16 widening"),
    );
    assert_eq!(
        exact_integer_cast_obligation(
            wide_type,
            narrow_type,
            widened,
            std::slice::from_ref(&definition),
            &BTreeSet::from([input_id]),
        ),
        Proposition::Truth
    );
}

#[test]
fn reconstructs_multiply_chain_exact_cast_bounds_and_rejects_broken_definitions() {
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let root_id = ValueId::new(201).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(u16_type));
    let first = ScalarTerm::value(
        ValueId::new(202).expect("first"),
        ScalarType::Integer(u16_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(203).expect("second"),
        ScalarType::Integer(u16_type),
    );
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_multiply(
            u16_type,
            root.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(2)).expect("2u16"),
        )
        .expect("root * 2u16"),
    );
    let second_definition = Proposition::Equal(
        second.clone(),
        ScalarTerm::exact_integer_multiply(
            u16_type,
            first.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(3)).expect("3u16"),
        )
        .expect("first * 3u16"),
    );
    let parameters = BTreeSet::from([root_id]);
    let expected = Proposition::LessOrEqual(
        root.clone(),
        ScalarTerm::integer(u16_type, IntegerValue::Unsigned(42)).expect("42u16"),
    );
    let reconstruct = |axioms: &[Proposition]| {
        exact_integer_cast_obligation(u16_type, u8_type, second.clone(), axioms, &parameters)
    };
    assert_eq!(
        exact_integer_cast_obligation(
            u16_type,
            u8_type,
            first.clone(),
            std::slice::from_ref(&first_definition),
            &parameters,
        ),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(127)).expect("127u16"),
        )
    );
    assert_eq!(
        reconstruct(&[first_definition.clone(), second_definition.clone()]),
        expected
    );
    let one = ScalarTerm::value(
        ValueId::new(204).expect("one"),
        ScalarType::Integer(u16_type),
    );
    let one_definition = Proposition::Equal(
        one.clone(),
        ScalarTerm::exact_integer_multiply(
            u16_type,
            root.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(1)).expect("1u16"),
        )
        .expect("root * 1u16"),
    );
    assert_eq!(
        exact_integer_cast_obligation(
            u16_type,
            u8_type,
            one,
            std::slice::from_ref(&one_definition),
            &parameters,
        ),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(255)).expect("255u16"),
        )
    );
    let zero = ScalarTerm::value(
        ValueId::new(205).expect("zero"),
        ScalarType::Integer(u16_type),
    );
    let zero_definition = Proposition::Equal(
        zero.clone(),
        ScalarTerm::exact_integer_multiply(
            u16_type,
            first.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(0)).expect("0u16"),
        )
        .expect("first * 0u16"),
    );
    assert_eq!(
        exact_integer_cast_obligation(
            u16_type,
            u8_type,
            zero,
            &[first_definition.clone(), zero_definition],
            &parameters,
        ),
        Proposition::Truth,
        "a zero cumulative product discharges only the cast; the prefix multiply keeps its own proof"
    );

    assert_ne!(
        reconstruct(&[second_definition.clone(), first_definition.clone()]),
        expected
    );
    let reversed_second = match second_definition.clone() {
        Proposition::Equal(left, right) => Proposition::Equal(right, left),
        _ => unreachable!("definition is an equality"),
    };
    assert_ne!(
        reconstruct(&[first_definition.clone(), reversed_second]),
        expected
    );
    let literal_left_first = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_multiply(
            u16_type,
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(2)).expect("2u16"),
            root.clone(),
        )
        .expect("2u16 * root"),
    );
    assert_ne!(
        reconstruct(&[literal_left_first, second_definition.clone()]),
        expected
    );
    let runtime_factor = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_multiply(u16_type, root.clone(), root.clone())
            .expect("root * root"),
    );
    assert_ne!(
        reconstruct(&[runtime_factor, second_definition.clone()]),
        expected
    );
    assert_ne!(
        reconstruct(&[first_definition.clone(), second_definition.clone()]),
        exact_integer_cast_obligation(
            u16_type,
            u8_type,
            second.clone(),
            &[first_definition.clone(), second_definition.clone()],
            &BTreeSet::new(),
        )
    );

    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_root_id = ValueId::new(211).expect("signed root");
    let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(i16_type));
    let signed_first = ScalarTerm::value(
        ValueId::new(212).expect("signed first"),
        ScalarType::Integer(i16_type),
    );
    let signed_second = ScalarTerm::value(
        ValueId::new(213).expect("signed second"),
        ScalarType::Integer(i16_type),
    );
    let signed_definitions = [
        Proposition::Equal(
            signed_first.clone(),
            ScalarTerm::exact_integer_multiply(
                i16_type,
                signed_root.clone(),
                ScalarTerm::integer(i16_type, IntegerValue::Signed(2)).expect("2i16"),
            )
            .expect("signed root * 2i16"),
        ),
        Proposition::Equal(
            signed_second.clone(),
            ScalarTerm::exact_integer_multiply(
                i16_type,
                signed_first,
                ScalarTerm::integer(i16_type, IntegerValue::Signed(3)).expect("3i16"),
            )
            .expect("signed first * 3i16"),
        ),
    ];
    assert_eq!(
        exact_integer_cast_obligation(
            i16_type,
            i8_type,
            signed_second,
            &signed_definitions,
            &BTreeSet::from([signed_root_id]),
        ),
        canonical_conjunction(vec![
            Proposition::LessOrEqual(
                ScalarTerm::integer(i16_type, IntegerValue::Signed(-21)).expect("-21i16"),
                signed_root.clone(),
            ),
            Proposition::LessOrEqual(
                signed_root,
                ScalarTerm::integer(i16_type, IntegerValue::Signed(21)).expect("21i16"),
            ),
        ])
    );

    let cross_signed_root_id = ValueId::new(221).expect("signed cross root");
    let cross_signed_root = ScalarTerm::value(cross_signed_root_id, ScalarType::Integer(i8_type));
    let cross_signed_product = ScalarTerm::value(
        ValueId::new(222).expect("signed cross product"),
        ScalarType::Integer(i8_type),
    );
    let cross_signed_definition = Proposition::Equal(
        cross_signed_product.clone(),
        ScalarTerm::exact_integer_multiply(
            i8_type,
            cross_signed_root.clone(),
            ScalarTerm::integer(i8_type, IntegerValue::Signed(2)).expect("2i8"),
        )
        .expect("signed cross root * 2i8"),
    );
    assert_eq!(
        exact_integer_cast_obligation(
            i8_type,
            u8_type,
            cross_signed_product,
            std::slice::from_ref(&cross_signed_definition),
            &BTreeSet::from([cross_signed_root_id]),
        ),
        Proposition::LessOrEqual(
            ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).expect("0i8"),
            cross_signed_root,
        )
    );

    let cross_unsigned_root_id = ValueId::new(231).expect("unsigned cross root");
    let cross_unsigned_root =
        ScalarTerm::value(cross_unsigned_root_id, ScalarType::Integer(u8_type));
    let cross_unsigned_product = ScalarTerm::value(
        ValueId::new(232).expect("unsigned cross product"),
        ScalarType::Integer(u8_type),
    );
    let cross_unsigned_definition = Proposition::Equal(
        cross_unsigned_product.clone(),
        ScalarTerm::exact_integer_multiply(
            u8_type,
            cross_unsigned_root.clone(),
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(2)).expect("2u8"),
        )
        .expect("unsigned cross root * 2u8"),
    );
    assert_eq!(
        exact_integer_cast_obligation(
            u8_type,
            i8_type,
            cross_unsigned_product,
            std::slice::from_ref(&cross_unsigned_definition),
            &BTreeSet::from([cross_unsigned_root_id]),
        ),
        Proposition::LessOrEqual(
            cross_unsigned_root,
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(63)).expect("63u8"),
        )
    );

    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let overflow_root_id = ValueId::new(241).expect("overflow root");
    let overflow_root = ScalarTerm::value(overflow_root_id, ScalarType::Integer(u64_type));
    let overflow_first = ScalarTerm::value(
        ValueId::new(242).expect("overflow first"),
        ScalarType::Integer(u64_type),
    );
    let overflow_second = ScalarTerm::value(
        ValueId::new(243).expect("overflow second"),
        ScalarType::Integer(u64_type),
    );
    let overflow_third = ScalarTerm::value(
        ValueId::new(244).expect("overflow third"),
        ScalarType::Integer(u64_type),
    );
    let maximum =
        ScalarTerm::integer(u64_type, IntegerValue::Unsigned(u64::MAX as u128)).expect("u64::MAX");
    let overflow_definitions = [
        Proposition::Equal(
            overflow_first.clone(),
            ScalarTerm::exact_integer_multiply(u64_type, overflow_root, maximum.clone())
                .expect("overflow first"),
        ),
        Proposition::Equal(
            overflow_second.clone(),
            ScalarTerm::exact_integer_multiply(u64_type, overflow_first, maximum.clone())
                .expect("overflow second"),
        ),
        Proposition::Equal(
            overflow_third.clone(),
            ScalarTerm::exact_integer_multiply(u64_type, overflow_second, maximum)
                .expect("overflow third"),
        ),
    ];
    assert_eq!(
        exact_integer_cast_obligation(
            u64_type,
            i64_type,
            overflow_third,
            &overflow_definitions,
            &BTreeSet::from([overflow_root_id]),
        ),
        Proposition::Falsehood
    );
}

#[test]
fn reconstructs_affine_chain_exact_cast_bounds_zero_collapse_and_fences() {
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let root_id = ValueId::new(251).expect("root");
    let value = |id| {
        ScalarTerm::value(
            ValueId::new(id).expect("value"),
            ScalarType::Integer(u16_type),
        )
    };
    let root = ScalarTerm::value(root_id, ScalarType::Integer(u16_type));
    let first = value(252);
    let second = value(253);
    let third = value(254);
    let definitions = [
        Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_add(
                u16_type,
                root.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(3)).expect("3u16"),
            )
            .expect("root + 3"),
        ),
        Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_multiply(
                u16_type,
                first.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(2)).expect("2u16"),
            )
            .expect("first * 2"),
        ),
        Proposition::Equal(
            third.clone(),
            ScalarTerm::exact_integer_subtract(
                u16_type,
                second.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(1)).expect("1u16"),
            )
            .expect("second - 1"),
        ),
    ];
    let parameters = BTreeSet::from([root_id]);
    let expected = Proposition::LessOrEqual(
        root.clone(),
        ScalarTerm::integer(u16_type, IntegerValue::Unsigned(125)).expect("125u16"),
    );
    let reconstruct = |axioms: &[Proposition]| {
        exact_integer_cast_obligation(u16_type, u8_type, third.clone(), axioms, &parameters)
    };
    assert_eq!(reconstruct(&definitions), expected);
    assert_ne!(
        reconstruct(&[
            definitions[1].clone(),
            definitions[0].clone(),
            definitions[2].clone(),
        ]),
        expected,
    );
    assert_eq!(
        exact_integer_affine_chain_cast_obligation(
            u16_type,
            u8_type,
            first.clone(),
            &definitions[..1],
            &parameters,
        ),
        None,
        "homogeneous offset chains remain on the narrower cast path",
    );

    let zero = value(255);
    let constant = value(256);
    let zero_definitions = [
        definitions[0].clone(),
        Proposition::Equal(
            zero.clone(),
            ScalarTerm::exact_integer_multiply(
                u16_type,
                first.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(0)).expect("0u16"),
            )
            .expect("first * 0"),
        ),
        Proposition::Equal(
            constant.clone(),
            ScalarTerm::exact_integer_add(
                u16_type,
                zero.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(255)).expect("255u16"),
            )
            .expect("zero + 255"),
        ),
    ];
    assert_eq!(
        exact_integer_cast_obligation(u16_type, u8_type, constant, &zero_definitions, &parameters,),
        Proposition::Truth,
        "a zero coefficient discharges only the cast when its offset fits the target",
    );
    let outside = value(257);
    let outside_definition = Proposition::Equal(
        outside.clone(),
        ScalarTerm::exact_integer_add(
            u16_type,
            zero,
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(256)).expect("256u16"),
        )
        .expect("zero + 256"),
    );
    assert_eq!(
        exact_integer_cast_obligation(
            u16_type,
            u8_type,
            outside,
            &[
                zero_definitions[0].clone(),
                zero_definitions[1].clone(),
                outside_definition,
            ],
            &parameters,
        ),
        Proposition::Falsehood,
        "a zero coefficient still checks target representability of its offset",
    );

    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let signed_root_id = ValueId::new(261).expect("signed root");
    let signed_value = |id| {
        ScalarTerm::value(
            ValueId::new(id).expect("signed value"),
            ScalarType::Integer(i16_type),
        )
    };
    let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(i16_type));
    let signed_first = signed_value(262);
    let signed_second = signed_value(263);
    let signed_third = signed_value(264);
    let signed_definitions = [
        Proposition::Equal(
            signed_first.clone(),
            ScalarTerm::exact_integer_subtract(
                i16_type,
                signed_root.clone(),
                ScalarTerm::integer(i16_type, IntegerValue::Signed(3)).expect("3i16"),
            )
            .expect("root - 3"),
        ),
        Proposition::Equal(
            signed_second.clone(),
            ScalarTerm::exact_integer_multiply(
                i16_type,
                signed_first,
                ScalarTerm::integer(i16_type, IntegerValue::Signed(2)).expect("2i16"),
            )
            .expect("first * 2"),
        ),
        Proposition::Equal(
            signed_third.clone(),
            ScalarTerm::exact_integer_add(
                i16_type,
                signed_second,
                ScalarTerm::integer(i16_type, IntegerValue::Signed(1)).expect("1i16"),
            )
            .expect("second + 1"),
        ),
    ];
    assert_eq!(
        exact_integer_cast_obligation(
            i16_type,
            u8_type,
            signed_third,
            &signed_definitions,
            &BTreeSet::from([signed_root_id]),
        ),
        canonical_conjunction(vec![
            Proposition::LessOrEqual(
                ScalarTerm::integer(i16_type, IntegerValue::Signed(3)).expect("3i16"),
                signed_root.clone(),
            ),
            Proposition::LessOrEqual(
                signed_root,
                ScalarTerm::integer(i16_type, IntegerValue::Signed(130)).expect("130i16"),
            ),
        ]),
    );

    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let overflow_root_id = ValueId::new(271).expect("overflow root");
    let overflow_value = |id| {
        ScalarTerm::value(
            ValueId::new(id).expect("overflow value"),
            ScalarType::Integer(u64_type),
        )
    };
    let overflow_root = ScalarTerm::value(overflow_root_id, ScalarType::Integer(u64_type));
    let overflow_offset = overflow_value(272);
    let overflow_first = overflow_value(273);
    let overflow_second = overflow_value(274);
    let overflow_third = overflow_value(275);
    let maximum = ScalarTerm::integer(u64_type, IntegerValue::Unsigned(u128::from(u64::MAX)))
        .expect("u64::MAX");
    let overflow_definitions = [
        Proposition::Equal(
            overflow_offset.clone(),
            ScalarTerm::exact_integer_add(
                u64_type,
                overflow_root,
                ScalarTerm::integer(u64_type, IntegerValue::Unsigned(1)).expect("1u64"),
            )
            .expect("root + 1"),
        ),
        Proposition::Equal(
            overflow_first.clone(),
            ScalarTerm::exact_integer_multiply(u64_type, overflow_offset, maximum.clone())
                .expect("offset * MAX"),
        ),
        Proposition::Equal(
            overflow_second.clone(),
            ScalarTerm::exact_integer_multiply(u64_type, overflow_first, maximum.clone())
                .expect("first * MAX"),
        ),
        Proposition::Equal(
            overflow_third.clone(),
            ScalarTerm::exact_integer_multiply(u64_type, overflow_second, maximum)
                .expect("second * MAX"),
        ),
    ];
    assert_eq!(
        exact_integer_cast_obligation(
            u64_type,
            i64_type,
            overflow_third,
            &overflow_definitions,
            &BTreeSet::from([overflow_root_id]),
        ),
        Proposition::Falsehood,
        "coefficient or offset composition overflow fails closed",
    );
}
