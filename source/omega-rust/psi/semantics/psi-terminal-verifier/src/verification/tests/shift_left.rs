use super::super::*;
use psi_core::{IntegerType, ScalarType, ValueId};

#[test]
fn exact_shift_left_chain_reconstructs_cumulative_parameter_bounds() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value");
    let i8_count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
    let u16_count_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
    let i32_count_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 count");
    let root = ScalarTerm::value(ValueId::new(1).expect("root"), ScalarType::Integer(u8_type));
    let first = ScalarTerm::value(
        ValueId::new(2).expect("first"),
        ScalarType::Integer(u8_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(3).expect("second"),
        ScalarType::Integer(u8_type),
    );
    let one = ScalarTerm::integer(i8_count_type, IntegerValue::Signed(1)).expect("1i8");
    let two = ScalarTerm::integer(u16_count_type, IntegerValue::Unsigned(2)).expect("2u16");
    let zero = ScalarTerm::integer(i32_count_type, IntegerValue::Signed(0)).expect("0i32");
    let axioms = vec![
        Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_left(u8_type, i8_count_type, root.clone(), one)
                .expect("root << 1i8"),
        ),
        Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_shift_left(u8_type, u16_count_type, first, two)
                .expect("first << 2u16"),
        ),
    ];
    let thirty_one = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(31)).expect("31u8");
    assert_eq!(
        exact_integer_shift_left_obligation(
            u8_type,
            i32_count_type,
            second,
            zero,
            &axioms,
            axioms.len(),
            &BTreeSet::from([ValueId::new(1).expect("root")]),
        ),
        Proposition::LessOrEqual(root, thirty_one)
    );

    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 value");
    let signed_root = ScalarTerm::value(
        ValueId::new(4).expect("signed root"),
        ScalarType::Integer(i8_type),
    );
    let signed_first = ScalarTerm::value(
        ValueId::new(5).expect("signed first"),
        ScalarType::Integer(i8_type),
    );
    let one = ScalarTerm::integer(i8_count_type, IntegerValue::Signed(1)).expect("1i8");
    let two = ScalarTerm::integer(u16_count_type, IntegerValue::Unsigned(2)).expect("2u16");
    let signed_axioms = vec![Proposition::Equal(
        signed_first.clone(),
        ScalarTerm::exact_integer_shift_left(i8_type, i8_count_type, signed_root.clone(), one)
            .expect("signed root << 1i8"),
    )];
    let negative_sixteen = ScalarTerm::integer(i8_type, IntegerValue::Signed(-16)).expect("-16i8");
    let fifteen = ScalarTerm::integer(i8_type, IntegerValue::Signed(15)).expect("15i8");
    assert_eq!(
        exact_integer_shift_left_obligation(
            i8_type,
            u16_count_type,
            signed_first,
            two,
            &signed_axioms,
            signed_axioms.len(),
            &BTreeSet::from([ValueId::new(4).expect("signed root")]),
        ),
        canonical_conjunction(vec![
            Proposition::LessOrEqual(negative_sixteen, signed_root.clone()),
            Proposition::LessOrEqual(signed_root, fifteen),
        ])
    );
}

#[test]
fn exact_shift_left_chain_rejects_broken_definitions_and_counts() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 value");
    let count_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 count");
    let signed_count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
    let root_id = ValueId::new(1).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(u8_type));
    let first = ScalarTerm::value(
        ValueId::new(2).expect("first"),
        ScalarType::Integer(u8_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(3).expect("second"),
        ScalarType::Integer(u8_type),
    );
    let local = ScalarTerm::value(
        ValueId::new(4).expect("local"),
        ScalarType::Integer(u8_type),
    );
    let one = ScalarTerm::integer(count_type, IntegerValue::Unsigned(1)).expect("1u8");
    let two = ScalarTerm::integer(count_type, IntegerValue::Unsigned(2)).expect("2u8");
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_shift_left(u8_type, count_type, root.clone(), one.clone())
            .expect("root << 1u8"),
    );
    let second_definition = Proposition::Equal(
        second.clone(),
        ScalarTerm::exact_integer_shift_left(u8_type, count_type, first.clone(), two.clone())
            .expect("first << 2u8"),
    );
    let parameters = BTreeSet::from([root_id]);
    let reconstruct = |axioms: &[Proposition], parameters: &BTreeSet<ValueId>| {
        exact_integer_shift_left_chain_obligation(
            u8_type,
            second.clone(),
            0,
            axioms,
            axioms.len(),
            parameters,
        )
    };
    assert_eq!(
        reconstruct(
            &[first_definition.clone(), second_definition.clone()],
            &parameters,
        ),
        Some(Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(31)).expect("31u8"),
        ))
    );
    assert_eq!(
        reconstruct(
            &[second_definition.clone(), first_definition.clone()],
            &parameters,
        ),
        None,
        "a definition may not be recovered from outside the shrinking prefix"
    );

    let reversed_first = Proposition::Equal(
        ScalarTerm::exact_integer_shift_left(u8_type, count_type, root.clone(), one.clone())
            .expect("root << 1u8"),
        first.clone(),
    );
    assert_eq!(
        reconstruct(&[reversed_first, second_definition.clone()], &parameters),
        None,
        "a symmetric equality is not operation-definition authority"
    );

    let redirected_second = Proposition::Equal(
        second.clone(),
        ScalarTerm::exact_integer_shift_left(u8_type, count_type, local.clone(), two.clone())
            .expect("local << 2u8"),
    );
    assert_eq!(
        reconstruct(&[first_definition.clone(), redirected_second], &parameters),
        None
    );
    assert_eq!(
        reconstruct(
            &[first_definition.clone(), second_definition.clone()],
            &BTreeSet::new(),
        ),
        None,
        "a local or block parameter is not a machine-parameter root"
    );

    let cyclic_first = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_shift_left(u8_type, count_type, second.clone(), one.clone())
            .expect("second << 1u8"),
    );
    assert_eq!(
        reconstruct(&[cyclic_first, second_definition.clone()], &parameters),
        None
    );

    let negative = ScalarTerm::integer(signed_count_type, IntegerValue::Signed(-1)).expect("-1i8");
    let negative_first = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_shift_left(u8_type, signed_count_type, root.clone(), negative)
            .expect("root << -1i8 remains a proposition term"),
    );
    assert_eq!(
        reconstruct(&[negative_first, second_definition.clone()], &parameters),
        None
    );

    let eight = ScalarTerm::integer(count_type, IntegerValue::Unsigned(8)).expect("8u8");
    let out_of_range_first = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_shift_left(u8_type, count_type, root.clone(), eight)
            .expect("root << 8u8 remains a proposition term"),
    );
    assert_eq!(
        reconstruct(
            &[out_of_range_first, second_definition.clone()],
            &parameters,
        ),
        None
    );

    let computed_count = ScalarTerm::value(
        ValueId::new(5).expect("computed count"),
        ScalarType::Integer(count_type),
    );
    let computed_first = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_shift_left(u8_type, count_type, root.clone(), computed_count)
            .expect("root << computed count"),
    );
    assert_eq!(
        reconstruct(&[computed_first, second_definition.clone()], &parameters),
        None
    );

    let signed_root = ScalarTerm::value(
        ValueId::new(6).expect("signed root"),
        ScalarType::Integer(i8_type),
    );
    let mismatched_first = Proposition::Equal(
        first,
        ScalarTerm::exact_integer_shift_left(
            i8_type,
            signed_count_type,
            signed_root,
            ScalarTerm::integer(signed_count_type, IntegerValue::Signed(1)).expect("1i8"),
        )
        .expect("signed root << 1i8"),
    );
    assert_eq!(
        reconstruct(&[mismatched_first, second_definition], &parameters),
        None
    );
}

#[test]
fn exact_shift_left_chain_handles_zero_width_and_count_overflow() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value");
    let count_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 count");
    let root = ScalarTerm::value(ValueId::new(1).expect("root"), ScalarType::Integer(u8_type));
    let first = ScalarTerm::value(
        ValueId::new(2).expect("first"),
        ScalarType::Integer(u8_type),
    );
    let four = ScalarTerm::integer(count_type, IntegerValue::Unsigned(4)).expect("4u8");
    let zero = ScalarTerm::integer(count_type, IntegerValue::Unsigned(0)).expect("0u8");
    let width_axioms = vec![Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_shift_left(u8_type, count_type, root.clone(), four.clone())
            .expect("root << 4u8"),
    )];
    let zero_value = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(0)).expect("0u8");
    assert_eq!(
        exact_integer_shift_left_obligation(
            u8_type,
            count_type,
            first.clone(),
            four,
            &width_axioms,
            width_axioms.len(),
            &BTreeSet::from([ValueId::new(1).expect("root")]),
        ),
        Proposition::LessOrEqual(root.clone(), zero_value)
    );
    let identity_axioms = vec![Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_shift_left(u8_type, count_type, root, zero.clone())
            .expect("root << 0u8"),
    )];
    assert_eq!(
        exact_integer_shift_left_obligation(
            u8_type,
            count_type,
            first.clone(),
            zero,
            &identity_axioms,
            identity_axioms.len(),
            &BTreeSet::from([ValueId::new(1).expect("root")]),
        ),
        Proposition::Truth
    );
    assert_eq!(
        exact_integer_shift_left_chain_obligation(
            u8_type,
            first,
            u128::MAX,
            &width_axioms,
            width_axioms.len(),
            &BTreeSet::from([ValueId::new(1).expect("root")]),
        ),
        Some(Proposition::Falsehood)
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 value");
    let signed_root = ScalarTerm::value(
        ValueId::new(3).expect("signed root"),
        ScalarType::Integer(i8_type),
    );
    let signed_zero = ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).expect("0i8");
    assert_eq!(
        exact_integer_cumulative_shift_left_obligation(i8_type, signed_root.clone(), 8),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(signed_zero.clone(), signed_root.clone()),
            Proposition::LessOrEqual(signed_root, signed_zero),
        ])
    );
}

#[test]
fn exact_shift_left_chain_after_partial_cast_reconstructs_source_intersections() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let i8_count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
    let u16_count_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
    let i32_count_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 count");
    let root_id = ValueId::new(241).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
    let cast = ScalarTerm::value(
        ValueId::new(242).expect("cast"),
        ScalarType::Integer(target_type),
    );
    let first = ScalarTerm::value(
        ValueId::new(243).expect("first shift"),
        ScalarType::Integer(target_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(244).expect("second shift"),
        ScalarType::Integer(target_type),
    );
    let cast_definition = Proposition::Equal(
        cast.clone(),
        ScalarTerm::integer_exact_cast(source_type, target_type, root.clone())
            .expect("u16 to u8 exact cast"),
    );
    let one = ScalarTerm::integer(i8_count_type, IntegerValue::Signed(1)).expect("1i8");
    let two = ScalarTerm::integer(u16_count_type, IntegerValue::Unsigned(2)).expect("2u16");
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_shift_left(target_type, i8_count_type, cast.clone(), one.clone())
            .expect("cast << 1i8"),
    );
    let second_definition = Proposition::Equal(
        second.clone(),
        ScalarTerm::exact_integer_shift_left(
            target_type,
            u16_count_type,
            first.clone(),
            two.clone(),
        )
        .expect("(cast << 1i8) << 2u16"),
    );
    let parameters = BTreeSet::from([root_id]);
    assert_eq!(
        exact_integer_shift_left_obligation(
            target_type,
            i8_count_type,
            cast.clone(),
            one,
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
        exact_integer_shift_left_obligation(
            target_type,
            u16_count_type,
            first,
            two,
            &[cast_definition.clone(), first_definition.clone()],
            2,
            &parameters,
        ),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(31)).expect("31u16"),
        )
    );
    assert_eq!(
        exact_integer_shift_left_obligation(
            target_type,
            i32_count_type,
            second.clone(),
            ScalarTerm::integer(i32_count_type, IntegerValue::Signed(5)).expect("5i32"),
            &[
                cast_definition.clone(),
                first_definition.clone(),
                second_definition.clone(),
            ],
            3,
            &parameters,
        ),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(0)).expect("0u16"),
        ),
        "a cumulative count at the target width requires the source root to be zero"
    );
    assert_eq!(
        exact_integer_shift_left_obligation(
            target_type,
            i32_count_type,
            cast.clone(),
            ScalarTerm::integer(i32_count_type, IntegerValue::Signed(0)).expect("0i32"),
            std::slice::from_ref(&cast_definition),
            1,
            &parameters,
        ),
        Proposition::Truth
    );

    let signed_source = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let signed_target = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_root_id = ValueId::new(251).expect("signed root");
    let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(signed_source));
    let signed_cast = ScalarTerm::value(
        ValueId::new(252).expect("signed cast"),
        ScalarType::Integer(signed_target),
    );
    let signed_definition = Proposition::Equal(
        signed_cast.clone(),
        ScalarTerm::integer_exact_cast(signed_source, signed_target, signed_root.clone())
            .expect("i16 to i8 exact cast"),
    );
    assert_eq!(
        exact_integer_shift_left_obligation(
            signed_target,
            u16_count_type,
            signed_cast,
            ScalarTerm::integer(u16_count_type, IntegerValue::Unsigned(3)).expect("3u16"),
            std::slice::from_ref(&signed_definition),
            1,
            &BTreeSet::from([signed_root_id]),
        ),
        canonical_conjunction(vec![
            Proposition::LessOrEqual(
                ScalarTerm::integer(signed_source, IntegerValue::Signed(-16)).expect("-16i16"),
                signed_root.clone(),
            ),
            Proposition::LessOrEqual(
                signed_root,
                ScalarTerm::integer(signed_source, IntegerValue::Signed(15)).expect("15i16"),
            ),
        ])
    );

    let cross_source = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let cross_target = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let cross_root_id = ValueId::new(261).expect("cross root");
    let cross_root = ScalarTerm::value(cross_root_id, ScalarType::Integer(cross_source));
    let cross_cast = ScalarTerm::value(
        ValueId::new(262).expect("cross cast"),
        ScalarType::Integer(cross_target),
    );
    let cross_definition = Proposition::Equal(
        cross_cast.clone(),
        ScalarTerm::integer_exact_cast(cross_source, cross_target, cross_root.clone())
            .expect("i8 to u8 exact cast"),
    );
    assert_eq!(
        exact_integer_shift_left_obligation(
            cross_target,
            i8_count_type,
            cross_cast,
            ScalarTerm::integer(i8_count_type, IntegerValue::Signed(1)).expect("1i8"),
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
        exact_integer_shift_left_obligation(
            target_type,
            i8_count_type,
            cast,
            ScalarTerm::integer(i8_count_type, IntegerValue::Signed(1)).expect("1i8"),
            std::slice::from_ref(&reversed_cast_definition),
            1,
            &parameters,
        ),
        Proposition::LessOrEqual(
            root,
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(127)).expect("127u16"),
        )
    );
    assert_eq!(
        exact_integer_cast_then_shift_left_chain_obligation(
            target_type,
            second,
            u128::MAX,
            &[cast_definition, first_definition, second_definition],
            3,
            &parameters,
        ),
        Some(Proposition::Falsehood),
        "cumulative-count overflow fails closed"
    );
}
