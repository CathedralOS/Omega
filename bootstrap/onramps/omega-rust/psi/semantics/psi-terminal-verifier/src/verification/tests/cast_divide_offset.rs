use super::super::*;
use psi_core::{IntegerType, ScalarType, ValueId};

#[test]
fn reconstructs_carrier_total_divide_remainder_chain_exact_casts() {
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let root_id = ValueId::new(451).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(u16_type));
    let first = ScalarTerm::value(
        ValueId::new(452).expect("first"),
        ScalarType::Integer(u16_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(453).expect("second"),
        ScalarType::Integer(u16_type),
    );
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_divide(
            u16_type,
            root.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(2)).expect("2u16"),
        )
        .expect("root / 2u16"),
    );
    let second_definition = Proposition::Equal(
        second.clone(),
        ScalarTerm::exact_integer_remainder(
            u16_type,
            first.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(3)).expect("3u16"),
        )
        .expect("first % 3u16"),
    );
    let parameters = BTreeSet::from([root_id]);
    assert_eq!(
        exact_integer_cast_obligation(
            u16_type,
            u8_type,
            second.clone(),
            &[first_definition.clone(), second_definition.clone()],
            &parameters,
        ),
        Proposition::Truth,
    );
    assert_ne!(
        exact_integer_cast_obligation(
            u16_type,
            u8_type,
            second,
            &[second_definition, first_definition.clone()],
            &parameters,
        ),
        Proposition::Truth,
        "out-of-order definitions do not authorize the cast",
    );
    assert_ne!(
        exact_integer_cast_obligation(
            u16_type,
            u8_type,
            first,
            std::slice::from_ref(&first_definition),
            &parameters,
        ),
        Proposition::Truth,
        "a noncontained quotient hull stays outside the carrier-total family",
    );

    let signed_root_id = ValueId::new(461).expect("signed root");
    let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(i16_type));
    let signed_result = ScalarTerm::value(
        ValueId::new(462).expect("signed result"),
        ScalarType::Integer(i16_type),
    );
    let signed_definition = Proposition::Equal(
        signed_result.clone(),
        ScalarTerm::exact_integer_remainder(
            i16_type,
            signed_root,
            ScalarTerm::integer(i16_type, IntegerValue::Signed(-3)).expect("-3i16"),
        )
        .expect("signed root % -3i16"),
    );
    assert_eq!(
        exact_integer_cast_obligation(
            i16_type,
            i8_type,
            signed_result,
            std::slice::from_ref(&signed_definition),
            &BTreeSet::from([signed_root_id]),
        ),
        Proposition::Truth,
    );
    assert_eq!(
        exact_integer_cast_obligation(
            u16_type,
            i8_type,
            ScalarTerm::value(
                ValueId::new(463).expect("cross result"),
                ScalarType::Integer(u16_type),
            ),
            &[Proposition::Equal(
                ScalarTerm::value(
                    ValueId::new(463).expect("cross result"),
                    ScalarType::Integer(u16_type),
                ),
                ScalarTerm::exact_integer_remainder(
                    u16_type,
                    root,
                    ScalarTerm::integer(u16_type, IntegerValue::Unsigned(3)).expect("3u16"),
                )
                .expect("root % 3u16"),
            )],
            &parameters,
        ),
        Proposition::Truth,
        "a nonnegative remainder hull may cross to a signed target",
    );
}

#[test]
fn reconstructs_offset_chain_exact_cast_bounds_and_rejects_broken_definitions() {
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let root_id = ValueId::new(101).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(u16_type));
    let first = ScalarTerm::value(
        ValueId::new(102).expect("first"),
        ScalarType::Integer(u16_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(103).expect("second"),
        ScalarType::Integer(u16_type),
    );
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_add(
            u16_type,
            root.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(5)).expect("5u16"),
        )
        .expect("root + 5u16"),
    );
    let second_definition = Proposition::Equal(
        second.clone(),
        ScalarTerm::exact_integer_subtract(
            u16_type,
            first.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(3)).expect("3u16"),
        )
        .expect("first - 3u16"),
    );
    let parameters = BTreeSet::from([root_id]);
    let expected = Proposition::LessOrEqual(
        root.clone(),
        ScalarTerm::integer(u16_type, IntegerValue::Unsigned(253)).expect("253u16"),
    );
    let reconstruct = |axioms: &[Proposition]| {
        exact_integer_cast_obligation(u16_type, u8_type, second.clone(), axioms, &parameters)
    };
    assert_eq!(
        reconstruct(&[first_definition.clone(), second_definition.clone()]),
        expected
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
    let redirected_second = Proposition::Equal(
        second.clone(),
        ScalarTerm::integer(u16_type, IntegerValue::Unsigned(2)).expect("2u16"),
    );
    assert_ne!(
        reconstruct(&[first_definition.clone(), redirected_second]),
        expected
    );
    let cyclic_first = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_add(
            u16_type,
            second.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(5)).expect("5u16"),
        )
        .expect("second + 5u16"),
    );
    assert_ne!(
        reconstruct(&[cyclic_first, second_definition.clone()]),
        expected
    );
    let literal_left_first = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_add(
            u16_type,
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(5)).expect("5u16"),
            root.clone(),
        )
        .expect("5u16 + root"),
    );
    assert_ne!(
        reconstruct(&[literal_left_first, second_definition.clone()]),
        expected
    );
    assert_ne!(
        exact_integer_cast_obligation(
            u16_type,
            u8_type,
            second.clone(),
            &[first_definition.clone(), second_definition.clone()],
            &BTreeSet::new(),
        ),
        expected
    );

    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_root_id = ValueId::new(111).expect("signed root");
    let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(i16_type));
    let signed_first = ScalarTerm::value(
        ValueId::new(112).expect("signed first"),
        ScalarType::Integer(i16_type),
    );
    let signed_second = ScalarTerm::value(
        ValueId::new(113).expect("signed second"),
        ScalarType::Integer(i16_type),
    );
    let signed_definitions = [
        Proposition::Equal(
            signed_first.clone(),
            ScalarTerm::exact_integer_add(
                i16_type,
                signed_root.clone(),
                ScalarTerm::integer(i16_type, IntegerValue::Signed(5)).expect("5i16"),
            )
            .expect("signed root + 5i16"),
        ),
        Proposition::Equal(
            signed_second.clone(),
            ScalarTerm::exact_integer_subtract(
                i16_type,
                signed_first,
                ScalarTerm::integer(i16_type, IntegerValue::Signed(3)).expect("3i16"),
            )
            .expect("signed first - 3i16"),
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
                ScalarTerm::integer(i16_type, IntegerValue::Signed(-130)).expect("-130i16"),
                signed_root.clone(),
            ),
            Proposition::LessOrEqual(
                signed_root,
                ScalarTerm::integer(i16_type, IntegerValue::Signed(125)).expect("125i16"),
            ),
        ])
    );

    let i8_root_id = ValueId::new(121).expect("i8 root");
    let i8_root = ScalarTerm::value(i8_root_id, ScalarType::Integer(i8_type));
    let i8_offset = ScalarTerm::value(
        ValueId::new(122).expect("i8 offset"),
        ScalarType::Integer(i8_type),
    );
    let i8_definition = Proposition::Equal(
        i8_offset.clone(),
        ScalarTerm::exact_integer_subtract(
            i8_type,
            i8_root.clone(),
            ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
        )
        .expect("i8 root - 1i8"),
    );
    assert_eq!(
        exact_integer_cast_obligation(
            i8_type,
            u8_type,
            i8_offset,
            std::slice::from_ref(&i8_definition),
            &BTreeSet::from([i8_root_id]),
        ),
        Proposition::LessOrEqual(
            ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
            i8_root,
        )
    );

    let u8_root_id = ValueId::new(131).expect("u8 root");
    let u8_root = ScalarTerm::value(u8_root_id, ScalarType::Integer(u8_type));
    let impossible = ScalarTerm::value(
        ValueId::new(132).expect("impossible"),
        ScalarType::Integer(u8_type),
    );
    let impossible_definition = Proposition::Equal(
        impossible.clone(),
        ScalarTerm::exact_integer_add(
            u8_type,
            u8_root,
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(128)).expect("128u8"),
        )
        .expect("u8 root + 128u8"),
    );
    assert_eq!(
        exact_integer_cast_obligation(
            u8_type,
            i8_type,
            impossible,
            std::slice::from_ref(&impossible_definition),
            &BTreeSet::from([u8_root_id]),
        ),
        Proposition::Falsehood
    );

    let truth_root_id = ValueId::new(141).expect("truth root");
    let truth_root = ScalarTerm::value(truth_root_id, ScalarType::Integer(i8_type));
    let truth_first = ScalarTerm::value(
        ValueId::new(142).expect("truth first"),
        ScalarType::Integer(i8_type),
    );
    let truth_second = ScalarTerm::value(
        ValueId::new(143).expect("truth second"),
        ScalarType::Integer(i8_type),
    );
    let truth_definitions = [
        Proposition::Equal(
            truth_first.clone(),
            ScalarTerm::exact_integer_add(
                i8_type,
                truth_root,
                ScalarTerm::integer(i8_type, IntegerValue::Signed(127)).expect("127i8"),
            )
            .expect("truth root + 127i8"),
        ),
        Proposition::Equal(
            truth_second.clone(),
            ScalarTerm::exact_integer_add(
                i8_type,
                truth_first,
                ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("truth first + 1i8"),
        ),
    ];
    assert_eq!(
        exact_integer_cast_obligation(
            i8_type,
            u8_type,
            truth_second,
            &truth_definitions,
            &BTreeSet::from([truth_root_id]),
        ),
        Proposition::Truth,
        "the shifted target interval may cover the complete source carrier"
    );

    let overflow_root_id = ValueId::new(151).expect("overflow root");
    let overflow_root = ScalarTerm::value(overflow_root_id, ScalarType::Integer(u8_type));
    let overflow_first = ScalarTerm::value(
        ValueId::new(152).expect("overflow first"),
        ScalarType::Integer(u8_type),
    );
    let overflow_second = ScalarTerm::value(
        ValueId::new(153).expect("overflow second"),
        ScalarType::Integer(u8_type),
    );
    let overflow_definitions = [
        Proposition::Equal(
            overflow_first.clone(),
            ScalarTerm::exact_integer_add(
                u8_type,
                overflow_root,
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(200)).expect("200u8"),
            )
            .expect("overflow root + 200u8"),
        ),
        Proposition::Equal(
            overflow_second.clone(),
            ScalarTerm::exact_integer_add(
                u8_type,
                overflow_first,
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(100)).expect("100u8"),
            )
            .expect("overflow first + 100u8"),
        ),
    ];
    assert_eq!(
        exact_integer_cast_obligation(
            u8_type,
            i8_type,
            overflow_second,
            &overflow_definitions,
            &BTreeSet::from([overflow_root_id]),
        ),
        Proposition::Falsehood
    );
}
