use super::super::*;
use psi_core::{IntegerType, ScalarType, ValueId};

#[test]
fn reconstructs_shift_left_chain_exact_cast_bounds_and_rejects_broken_definitions() {
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let root_id = ValueId::new(301).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(u16_type));
    let first = ScalarTerm::value(
        ValueId::new(302).expect("first"),
        ScalarType::Integer(u16_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(303).expect("second"),
        ScalarType::Integer(u16_type),
    );
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_shift_left(
            u16_type,
            i8_type,
            root.clone(),
            ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
        )
        .expect("root << 1i8"),
    );
    let second_definition = Proposition::Equal(
        second.clone(),
        ScalarTerm::exact_integer_shift_left(
            u16_type,
            i32_type,
            first.clone(),
            ScalarTerm::integer(i32_type, IntegerValue::Signed(2)).expect("2i32"),
        )
        .expect("first << 2i32"),
    );
    let parameters = BTreeSet::from([root_id]);
    let expected = Proposition::LessOrEqual(
        root.clone(),
        ScalarTerm::integer(u16_type, IntegerValue::Unsigned(31)).expect("31u16"),
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
    let identity = ScalarTerm::value(
        ValueId::new(304).expect("identity"),
        ScalarType::Integer(u16_type),
    );
    let identity_definition = Proposition::Equal(
        identity.clone(),
        ScalarTerm::exact_integer_shift_left(
            u16_type,
            u8_type,
            root.clone(),
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(0)).expect("0u8"),
        )
        .expect("root << 0u8"),
    );
    assert_eq!(
        exact_integer_cast_obligation(
            u16_type,
            u8_type,
            identity,
            std::slice::from_ref(&identity_definition),
            &parameters,
        ),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(255)).expect("255u16"),
        )
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
    let runtime_count = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_shift_left(u16_type, u16_type, root.clone(), root.clone())
            .expect("root << root"),
    );
    assert_ne!(
        reconstruct(&[runtime_count, second_definition.clone()]),
        expected
    );
    let negative_count = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_shift_left(
            u16_type,
            i8_type,
            root.clone(),
            ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).expect("-1i8"),
        )
        .expect("root << -1i8 shape"),
    );
    assert_ne!(
        reconstruct(&[negative_count, second_definition.clone()]),
        expected
    );
    let width_count = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_shift_left(
            u16_type,
            u8_type,
            root.clone(),
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(16)).expect("16u8"),
        )
        .expect("root << 16u8 shape"),
    );
    assert_ne!(
        reconstruct(&[width_count, second_definition.clone()]),
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

    let signed_root_id = ValueId::new(311).expect("signed root");
    let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(i16_type));
    let signed_first = ScalarTerm::value(
        ValueId::new(312).expect("signed first"),
        ScalarType::Integer(i16_type),
    );
    let signed_second = ScalarTerm::value(
        ValueId::new(313).expect("signed second"),
        ScalarType::Integer(i16_type),
    );
    let signed_definitions = [
        Proposition::Equal(
            signed_first.clone(),
            ScalarTerm::exact_integer_shift_left(
                i16_type,
                u8_type,
                signed_root.clone(),
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).expect("1u8"),
            )
            .expect("signed root << 1u8"),
        ),
        Proposition::Equal(
            signed_second.clone(),
            ScalarTerm::exact_integer_shift_left(
                i16_type,
                i32_type,
                signed_first,
                ScalarTerm::integer(i32_type, IntegerValue::Signed(2)).expect("2i32"),
            )
            .expect("signed first << 2i32"),
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
                ScalarTerm::integer(i16_type, IntegerValue::Signed(-16)).expect("-16i16"),
                signed_root.clone(),
            ),
            Proposition::LessOrEqual(
                signed_root,
                ScalarTerm::integer(i16_type, IntegerValue::Signed(15)).expect("15i16"),
            ),
        ])
    );

    let signed_cross_root_id = ValueId::new(321).expect("signed cross root");
    let signed_cross_root = ScalarTerm::value(signed_cross_root_id, ScalarType::Integer(i8_type));
    let signed_cross_result = ScalarTerm::value(
        ValueId::new(322).expect("signed cross result"),
        ScalarType::Integer(i8_type),
    );
    let signed_cross_definition = Proposition::Equal(
        signed_cross_result.clone(),
        ScalarTerm::exact_integer_shift_left(
            i8_type,
            u16_type,
            signed_cross_root.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(1)).expect("1u16"),
        )
        .expect("signed cross root << 1u16"),
    );
    assert_eq!(
        exact_integer_cast_obligation(
            i8_type,
            u8_type,
            signed_cross_result,
            std::slice::from_ref(&signed_cross_definition),
            &BTreeSet::from([signed_cross_root_id]),
        ),
        Proposition::LessOrEqual(
            ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).expect("0i8"),
            signed_cross_root,
        )
    );

    let unsigned_cross_root_id = ValueId::new(331).expect("unsigned cross root");
    let unsigned_cross_root =
        ScalarTerm::value(unsigned_cross_root_id, ScalarType::Integer(u8_type));
    let unsigned_cross_result = ScalarTerm::value(
        ValueId::new(332).expect("unsigned cross result"),
        ScalarType::Integer(u8_type),
    );
    let unsigned_cross_definition = Proposition::Equal(
        unsigned_cross_result.clone(),
        ScalarTerm::exact_integer_shift_left(
            u8_type,
            i16_type,
            unsigned_cross_root.clone(),
            ScalarTerm::integer(i16_type, IntegerValue::Signed(1)).expect("1i16"),
        )
        .expect("unsigned cross root << 1i16"),
    );
    assert_eq!(
        exact_integer_cast_obligation(
            u8_type,
            i8_type,
            unsigned_cross_result,
            std::slice::from_ref(&unsigned_cross_definition),
            &BTreeSet::from([unsigned_cross_root_id]),
        ),
        Proposition::LessOrEqual(
            unsigned_cross_root,
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(63)).expect("63u8"),
        )
    );

    let narrow_zero_root_id = ValueId::new(341).expect("narrow zero root");
    let narrow_zero_root = ScalarTerm::value(narrow_zero_root_id, ScalarType::Integer(u16_type));
    let narrow_zero_result = ScalarTerm::value(
        ValueId::new(342).expect("narrow zero result"),
        ScalarType::Integer(u16_type),
    );
    let narrow_zero_definition = Proposition::Equal(
        narrow_zero_result.clone(),
        ScalarTerm::exact_integer_shift_left(
            u16_type,
            u8_type,
            narrow_zero_root.clone(),
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(8)).expect("8u8"),
        )
        .expect("narrow zero root << 8u8"),
    );
    assert_eq!(
        exact_integer_cast_obligation(
            u16_type,
            i8_type,
            narrow_zero_result,
            std::slice::from_ref(&narrow_zero_definition),
            &BTreeSet::from([narrow_zero_root_id]),
        ),
        Proposition::LessOrEqual(
            narrow_zero_root,
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(0)).expect("0u16"),
        ),
        "a sub-source-width shift beyond the target width independently narrows the cast interval to zero"
    );

    let width_root_id = ValueId::new(351).expect("width root");
    let width_root = ScalarTerm::value(width_root_id, ScalarType::Integer(u8_type));
    let width_first = ScalarTerm::value(
        ValueId::new(352).expect("width first"),
        ScalarType::Integer(u8_type),
    );
    let width_second = ScalarTerm::value(
        ValueId::new(353).expect("width second"),
        ScalarType::Integer(u8_type),
    );
    let width_definitions = [
        Proposition::Equal(
            width_first.clone(),
            ScalarTerm::exact_integer_shift_left(
                u8_type,
                u8_type,
                width_root,
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(4)).expect("4u8"),
            )
            .expect("width root << 4u8"),
        ),
        Proposition::Equal(
            width_second.clone(),
            ScalarTerm::exact_integer_shift_left(
                u8_type,
                i16_type,
                width_first,
                ScalarTerm::integer(i16_type, IntegerValue::Signed(4)).expect("4i16"),
            )
            .expect("width first << 4i16"),
        ),
    ];
    assert_eq!(
        exact_integer_cast_obligation(
            u8_type,
            i8_type,
            width_second,
            &width_definitions,
            &BTreeSet::from([width_root_id]),
        ),
        Proposition::Truth,
        "the cast of a successfully produced source-width exact shift result is zero-valued without importing either shift proof"
    );
}

#[test]
fn reconstructs_shift_right_chain_exact_cast_preimages_and_saturation() {
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let root_id = ValueId::new(401).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(u16_type));
    let first = ScalarTerm::value(
        ValueId::new(402).expect("first"),
        ScalarType::Integer(u16_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(403).expect("second"),
        ScalarType::Integer(u16_type),
    );
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_shift_right(
            u16_type,
            i8_type,
            root.clone(),
            ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
        )
        .expect("root >> 1i8"),
    );
    let second_definition = Proposition::Equal(
        second.clone(),
        ScalarTerm::exact_integer_shift_right(
            u16_type,
            u16_type,
            first.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(2)).expect("2u16"),
        )
        .expect("first >> 2u16"),
    );
    let parameters = BTreeSet::from([root_id]);
    let expected = Proposition::LessOrEqual(
        root.clone(),
        ScalarTerm::integer(u16_type, IntegerValue::Unsigned(2047)).expect("2047u16"),
    );
    assert_eq!(
        exact_integer_cast_obligation(
            u16_type,
            u8_type,
            second.clone(),
            &[first_definition.clone(), second_definition.clone()],
            &parameters,
        ),
        expected
    );
    assert_ne!(
        exact_integer_cast_obligation(
            u16_type,
            u8_type,
            second.clone(),
            &[second_definition.clone(), first_definition.clone()],
            &parameters,
        ),
        expected
    );
    let runtime_count = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_shift_right(u16_type, u16_type, root.clone(), root.clone())
            .expect("runtime count shape"),
    );
    assert_ne!(
        exact_integer_cast_obligation(
            u16_type,
            u8_type,
            second,
            &[runtime_count, second_definition],
            &parameters,
        ),
        expected
    );

    let signed_root = ScalarTerm::value(
        ValueId::new(411).expect("signed root"),
        ScalarType::Integer(i16_type),
    );
    assert_eq!(
        exact_integer_shift_right_chain_cast_interval_obligation(
            i16_type,
            i8_type,
            signed_root.clone(),
            3,
        ),
        canonical_conjunction(vec![
            Proposition::LessOrEqual(
                ScalarTerm::integer(i16_type, IntegerValue::Signed(-1024)).expect("-1024i16"),
                signed_root.clone(),
            ),
            Proposition::LessOrEqual(
                signed_root.clone(),
                ScalarTerm::integer(i16_type, IntegerValue::Signed(1023)).expect("1023i16"),
            ),
        ])
    );
    assert_eq!(
        exact_integer_shift_right_chain_cast_interval_obligation(
            i16_type,
            u8_type,
            signed_root.clone(),
            16,
        ),
        Proposition::LessOrEqual(
            ScalarTerm::integer(i16_type, IntegerValue::Signed(0)).expect("0i16"),
            signed_root,
        ),
        "signed saturation leaves -1 or 0, and an unsigned cast admits only the nonnegative root"
    );
    assert_eq!(
        exact_integer_shift_right_chain_cast_interval_obligation(u16_type, u8_type, root, 16,),
        Proposition::Truth,
        "unsigned saturation yields zero"
    );
    let unsigned_cross_root = ScalarTerm::value(
        ValueId::new(421).expect("unsigned cross root"),
        ScalarType::Integer(u8_type),
    );
    assert_eq!(
        exact_integer_shift_right_chain_cast_interval_obligation(
            u8_type,
            i8_type,
            unsigned_cross_root,
            1,
        ),
        Proposition::Truth,
        "one zero-fill shift makes every u8 result fit i8"
    );
}
