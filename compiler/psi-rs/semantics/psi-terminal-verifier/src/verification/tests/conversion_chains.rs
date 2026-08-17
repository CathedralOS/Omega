use super::super::*;
use psi_core::{IntegerType, ScalarType, ValueId};

#[test]
fn reconstructs_a_finite_exact_offset_chain_after_a_direct_partial_cast() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let root_id = ValueId::new(161).expect("root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
    let cast = ScalarTerm::value(
        ValueId::new(162).expect("cast"),
        ScalarType::Integer(target_type),
    );
    let cast_definition = Proposition::Equal(
        cast.clone(),
        ScalarTerm::integer_exact_cast(source_type, target_type, root.clone())
            .expect("u16 to u8 exact cast"),
    );
    let parameters = BTreeSet::from([root_id]);
    assert_eq!(
        exact_integer_cast_obligation(source_type, target_type, root.clone(), &[], &parameters,),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(255)).expect("255u16"),
        )
    );
    let five = ScalarTerm::integer(target_type, IntegerValue::Unsigned(5)).expect("5u8");
    let expected_add = Proposition::LessOrEqual(
        root.clone(),
        ScalarTerm::integer(source_type, IntegerValue::Unsigned(250)).expect("250u16"),
    );
    assert_eq!(
        exact_integer_add_obligation(
            target_type,
            cast.clone(),
            five.clone(),
            std::slice::from_ref(&cast_definition),
            1,
            &parameters,
        ),
        expected_add
    );
    let expected_subtract = canonical_conjunction(vec![
        Proposition::LessOrEqual(
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(5)).expect("5u16"),
            root.clone(),
        ),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(260)).expect("260u16"),
        ),
    ]);
    assert_eq!(
        exact_integer_subtract_obligation(
            target_type,
            cast.clone(),
            five.clone(),
            std::slice::from_ref(&cast_definition),
            1,
            &parameters,
        ),
        expected_subtract
    );

    let first = ScalarTerm::value(
        ValueId::new(163).expect("first offset"),
        ScalarType::Integer(target_type),
    );
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_add(target_type, cast.clone(), five.clone()).expect("cast + 5u8"),
    );
    let three = ScalarTerm::integer(target_type, IntegerValue::Unsigned(3)).expect("3u8");
    let second = ScalarTerm::value(
        ValueId::new(164).expect("second offset"),
        ScalarType::Integer(target_type),
    );
    let second_definition = Proposition::Equal(
        second.clone(),
        ScalarTerm::exact_integer_subtract(target_type, first.clone(), three.clone())
            .expect("(cast + 5u8) - 3u8"),
    );
    let two = ScalarTerm::integer(target_type, IntegerValue::Unsigned(2)).expect("2u8");
    let definitions = [
        cast_definition.clone(),
        first_definition.clone(),
        second_definition.clone(),
    ];
    assert_eq!(
        exact_integer_subtract_obligation(
            target_type,
            first.clone(),
            three,
            &definitions[..2],
            2,
            &parameters,
        ),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(253)).expect("253u16"),
        ),
        "the second prefix reconstructs cumulative offset +2"
    );
    assert_eq!(
        exact_integer_add_obligation(target_type, second, two, &definitions, 3, &parameters,),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(251)).expect("251u16"),
        ),
        "the third prefix reconstructs cumulative offset +4"
    );
    assert_eq!(
        exact_integer_subtract_obligation(
            target_type,
            first.clone(),
            five.clone(),
            &[cast_definition.clone(), first_definition.clone()],
            2,
            &parameters,
        ),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(255)).expect("255u16"),
        ),
        "later cancellation reconstructs its own zero-offset bound without replacing the first-prefix proof"
    );
    assert_ne!(
        exact_integer_add_obligation(
            target_type,
            first.clone(),
            ScalarTerm::integer(target_type, IntegerValue::Unsigned(2)).expect("2u8"),
            &[first_definition.clone(), cast_definition.clone()],
            2,
            &parameters,
        ),
        Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(248)).expect("248u16"),
        ),
        "definitions must precede the operation that consumes them"
    );

    let two_hundred = ScalarTerm::integer(target_type, IntegerValue::Unsigned(200)).expect("200u8");
    let large_first = ScalarTerm::value(
        ValueId::new(165).expect("large first offset"),
        ScalarType::Integer(target_type),
    );
    let large_first_definition = Proposition::Equal(
        large_first.clone(),
        ScalarTerm::exact_integer_add(target_type, cast.clone(), two_hundred)
            .expect("cast + 200u8"),
    );
    assert_eq!(
        exact_integer_add_obligation(
            target_type,
            large_first,
            ScalarTerm::integer(target_type, IntegerValue::Unsigned(100)).expect("100u8"),
            &[cast_definition.clone(), large_first_definition],
            2,
            &parameters,
        ),
        Proposition::Falsehood,
        "a cumulative offset wider than the target carrier fails closed"
    );

    let reversed_definition = match cast_definition.clone() {
        Proposition::Equal(left, right) => Proposition::Equal(right, left),
        _ => unreachable!("cast definition is an equality"),
    };
    assert_ne!(
        exact_integer_add_obligation(
            target_type,
            cast.clone(),
            five.clone(),
            std::slice::from_ref(&reversed_definition),
            1,
            &parameters,
        ),
        expected_add
    );
    let redirected_definition = Proposition::Equal(
        cast.clone(),
        ScalarTerm::integer(target_type, IntegerValue::Unsigned(1)).expect("1u8"),
    );
    assert_ne!(
        exact_integer_add_obligation(
            target_type,
            cast.clone(),
            five.clone(),
            std::slice::from_ref(&redirected_definition),
            1,
            &parameters,
        ),
        expected_add
    );
    assert_ne!(
        exact_integer_add_obligation(
            target_type,
            cast.clone(),
            five.clone(),
            std::slice::from_ref(&cast_definition),
            1,
            &BTreeSet::new(),
        ),
        expected_add
    );
    assert_ne!(
        exact_integer_add_obligation(
            target_type,
            five.clone(),
            cast,
            std::slice::from_ref(&cast_definition),
            1,
            &parameters,
        ),
        expected_add,
        "literal-left addition is outside the canonical composition"
    );

    let signed_source = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let signed_target = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_root_id = ValueId::new(171).expect("signed root");
    let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(signed_source));
    let signed_cast = ScalarTerm::value(
        ValueId::new(172).expect("signed cast"),
        ScalarType::Integer(signed_target),
    );
    let signed_definition = Proposition::Equal(
        signed_cast.clone(),
        ScalarTerm::integer_exact_cast(signed_source, signed_target, signed_root.clone())
            .expect("i16 to i8 exact cast"),
    );
    let negative_five = ScalarTerm::integer(signed_target, IntegerValue::Signed(-5)).expect("-5i8");
    assert_eq!(
        exact_integer_add_obligation(
            signed_target,
            signed_cast,
            negative_five,
            std::slice::from_ref(&signed_definition),
            1,
            &BTreeSet::from([signed_root_id]),
        ),
        canonical_conjunction(vec![
            Proposition::LessOrEqual(
                ScalarTerm::integer(signed_source, IntegerValue::Signed(-123)).expect("-123i16"),
                signed_root.clone(),
            ),
            Proposition::LessOrEqual(
                signed_root,
                ScalarTerm::integer(signed_source, IntegerValue::Signed(132)).expect("132i16"),
            ),
        ])
    );

    let cross_source = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let cross_target = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let cross_root_id = ValueId::new(181).expect("cross root");
    let cross_root = ScalarTerm::value(cross_root_id, ScalarType::Integer(cross_source));
    let cross_cast = ScalarTerm::value(
        ValueId::new(182).expect("cross cast"),
        ScalarType::Integer(cross_target),
    );
    let cross_definition = Proposition::Equal(
        cross_cast.clone(),
        ScalarTerm::integer_exact_cast(cross_source, cross_target, cross_root.clone())
            .expect("i8 to u8 exact cast"),
    );
    let cross_parameters = BTreeSet::from([cross_root_id]);
    assert_eq!(
        exact_integer_cast_obligation(
            cross_source,
            cross_target,
            cross_root.clone(),
            &[],
            &cross_parameters,
        ),
        Proposition::LessOrEqual(
            ScalarTerm::integer(cross_source, IntegerValue::Signed(0)).expect("0i8"),
            cross_root.clone(),
        )
    );
    assert_eq!(
        exact_integer_add_obligation(
            cross_target,
            cross_cast,
            ScalarTerm::integer(cross_target, IntegerValue::Unsigned(1)).expect("1u8"),
            std::slice::from_ref(&cross_definition),
            1,
            &cross_parameters,
        ),
        Proposition::LessOrEqual(
            ScalarTerm::integer(cross_source, IntegerValue::Signed(-1)).expect("-1i8"),
            cross_root,
        )
    );
}

#[test]
fn reconstructs_a_finite_ordered_widening_chain_and_rejects_broken_chains() {
    let narrow_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let middle_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let wide_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let deep_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let input_id = ValueId::new(1).expect("input");
    let input = ScalarTerm::value(input_id, ScalarType::Integer(narrow_type));
    let middle = ScalarTerm::value(
        ValueId::new(2).expect("middle"),
        ScalarType::Integer(middle_type),
    );
    let widened = ScalarTerm::value(
        ValueId::new(3).expect("widened"),
        ScalarType::Integer(wide_type),
    );
    let deeply_widened = ScalarTerm::value(
        ValueId::new(4).expect("deeply widened"),
        ScalarType::Integer(deep_type),
    );
    let middle_definition = Proposition::Equal(
        middle.clone(),
        ScalarTerm::integer_widen(narrow_type, middle_type, input).expect("u8 to u16 widening"),
    );
    let wide_definition = Proposition::Equal(
        widened.clone(),
        ScalarTerm::integer_widen(middle_type, wide_type, middle.clone())
            .expect("u16 to u32 widening"),
    );
    let deep_definition = Proposition::Equal(
        deeply_widened.clone(),
        ScalarTerm::integer_widen(wide_type, deep_type, widened.clone())
            .expect("u32 to u64 widening"),
    );
    let machine_parameter_values = BTreeSet::from([input_id]);
    assert_eq!(
        exact_integer_cast_obligation(
            deep_type,
            narrow_type,
            deeply_widened.clone(),
            &[
                middle_definition.clone(),
                wide_definition.clone(),
                deep_definition.clone(),
            ],
            &machine_parameter_values,
        ),
        Proposition::Truth
    );
    // A symmetric equality fact is not the verifier-owned operation
    // definition orientation.
    let reversed_deep_definition = match deep_definition.clone() {
        Proposition::Equal(left, right) => Proposition::Equal(right, left),
        _ => unreachable!("widen definition is an equality"),
    };
    assert_ne!(
        exact_integer_cast_obligation(
            deep_type,
            narrow_type,
            deeply_widened.clone(),
            &[
                middle_definition.clone(),
                wide_definition.clone(),
                reversed_deep_definition,
            ],
            &machine_parameter_values,
        ),
        Proposition::Truth
    );
    // Redirecting one result definition to a non-widening value breaks the
    // chain even though its surrounding carrier remains unchanged.
    let redirected_wide_definition = Proposition::Equal(
        widened.clone(),
        ScalarTerm::integer(wide_type, IntegerValue::Unsigned(0)).expect("0u32"),
    );
    assert_ne!(
        exact_integer_cast_obligation(
            deep_type,
            narrow_type,
            deeply_widened.clone(),
            &[
                middle_definition.clone(),
                redirected_wide_definition,
                deep_definition.clone(),
            ],
            &machine_parameter_values,
        ),
        Proposition::Truth
    );
    assert_ne!(
        exact_integer_cast_obligation(
            deep_type,
            narrow_type,
            deeply_widened.clone(),
            &[middle_definition.clone(), deep_definition.clone()],
            &machine_parameter_values,
        ),
        Proposition::Truth
    );
    assert_ne!(
        exact_integer_cast_obligation(
            deep_type,
            narrow_type,
            deeply_widened.clone(),
            &[
                deep_definition.clone(),
                middle_definition.clone(),
                wide_definition.clone(),
            ],
            &machine_parameter_values,
        ),
        Proposition::Truth
    );
    // A cycle cannot manufacture an origin: operation order decreases at
    // each step, and the malformed back-edge is also type-inconsistent.
    let cyclic_middle_definition = Proposition::Equal(
        middle,
        ScalarTerm::IntegerWiden {
            source_type: narrow_type,
            target_type: middle_type,
            operand: Box::new(deeply_widened.clone()),
        },
    );
    assert_ne!(
        exact_integer_cast_obligation(
            deep_type,
            narrow_type,
            deeply_widened.clone(),
            &[
                cyclic_middle_definition,
                wide_definition.clone(),
                deep_definition.clone(),
            ],
            &machine_parameter_values,
        ),
        Proposition::Truth
    );
    assert_ne!(
        exact_integer_cast_obligation(
            deep_type,
            narrow_type,
            deeply_widened,
            &[middle_definition, wide_definition, deep_definition],
            &BTreeSet::new(),
        ),
        Proposition::Truth
    );
}
