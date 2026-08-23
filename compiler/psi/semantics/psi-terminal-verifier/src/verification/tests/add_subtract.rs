use super::super::*;
use psi_core::{IntegerType, ScalarType, ValueId};

#[test]
fn reconstructs_unsigned_joint_exact_add_bounds() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::value(
        ValueId::new(1).expect("left"),
        ScalarType::Integer(integer_type),
    );
    let right = ScalarTerm::value(
        ValueId::new(2).expect("right"),
        ScalarType::Integer(integer_type),
    );
    let maximum = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(255)).expect("255u8");
    let remainder = ScalarTerm::exact_integer_subtract(integer_type, maximum, right.clone())
        .expect("255 - right");
    let bound = Proposition::LessOrEqual(left.clone(), remainder);
    assert_eq!(
        exact_integer_add_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            std::slice::from_ref(&bound),
            0,
            &BTreeSet::new(),
        ),
        bound.clone()
    );
}

#[test]
fn reconstructs_one_nested_exact_add_from_the_inner_result_definition() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let input = ScalarTerm::value(
        ValueId::new(1).expect("input"),
        ScalarType::Integer(integer_type),
    );
    let inner_result = ScalarTerm::value(
        ValueId::new(2).expect("inner result"),
        ScalarType::Integer(integer_type),
    );
    let one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).expect("1u8");
    let inner_definition = Proposition::Equal(
        inner_result.clone(),
        ScalarTerm::exact_integer_add(integer_type, input.clone(), one.clone())
            .expect("u8 exact add term"),
    );
    let machine_parameters = BTreeSet::from([ValueId::new(1).expect("input")]);
    assert_eq!(
        exact_integer_add_obligation(
            integer_type,
            inner_result,
            one,
            std::slice::from_ref(&inner_definition),
            1,
            &machine_parameters,
        ),
        Proposition::LessOrEqual(
            input,
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(253)).expect("253u8"),
        )
    );
}

#[test]
fn reconstructs_a_finite_ordered_exact_add_chain_and_rejects_broken_definitions() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let input_id = ValueId::new(1).expect("input");
    let input = ScalarTerm::value(input_id, ScalarType::Integer(integer_type));
    let first = ScalarTerm::value(
        ValueId::new(2).expect("first"),
        ScalarType::Integer(integer_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(3).expect("second"),
        ScalarType::Integer(integer_type),
    );
    let one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).expect("1u8");
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_add(integer_type, input.clone(), one.clone())
            .expect("first exact add"),
    );
    let second_definition = Proposition::Equal(
        second.clone(),
        ScalarTerm::exact_integer_add(integer_type, first.clone(), one.clone())
            .expect("second exact add"),
    );
    let parameters = BTreeSet::from([input_id]);
    let expected = Proposition::LessOrEqual(
        input,
        ScalarTerm::integer(integer_type, IntegerValue::Unsigned(252)).expect("252u8"),
    );
    let reconstruct = |axioms: &[Proposition], parameters: &BTreeSet<ValueId>| {
        exact_integer_add_obligation(
            integer_type,
            second.clone(),
            one.clone(),
            axioms,
            axioms.len(),
            parameters,
        )
    };
    assert_eq!(
        reconstruct(
            &[first_definition.clone(), second_definition.clone()],
            &parameters
        ),
        expected
    );
    assert_ne!(
        reconstruct(std::slice::from_ref(&second_definition), &parameters),
        expected
    );
    assert_ne!(
        reconstruct(
            &[second_definition.clone(), first_definition.clone()],
            &parameters
        ),
        expected
    );
    let reversed_second = match second_definition.clone() {
        Proposition::Equal(left, right) => Proposition::Equal(right, left),
        _ => unreachable!("exact-add definition is an equality"),
    };
    assert_ne!(
        reconstruct(&[first_definition.clone(), reversed_second], &parameters),
        expected
    );
    let redirected_second = Proposition::Equal(
        second.clone(),
        ScalarTerm::integer(integer_type, IntegerValue::Unsigned(2)).expect("2u8"),
    );
    assert_ne!(
        reconstruct(&[first_definition.clone(), redirected_second], &parameters),
        expected
    );
    let cyclic_first = Proposition::Equal(
        first,
        ScalarTerm::exact_integer_add(integer_type, second.clone(), one.clone())
            .expect("cyclic exact add"),
    );
    assert_ne!(
        reconstruct(&[cyclic_first, second_definition.clone()], &parameters),
        expected
    );
    assert_ne!(
        reconstruct(&[first_definition, second_definition], &BTreeSet::new(),),
        expected
    );
}

#[test]
fn reconstructs_wide_signed_offsets_cancellation_and_magnitude_overflow() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let input_id = ValueId::new(1).expect("input");
    let input = ScalarTerm::value(input_id, ScalarType::Integer(i8_type));
    let first = ScalarTerm::value(
        ValueId::new(2).expect("first"),
        ScalarType::Integer(i8_type),
    );
    let positive = ScalarTerm::integer(i8_type, IntegerValue::Signed(127)).expect("127i8");
    let negative = ScalarTerm::integer(i8_type, IntegerValue::Signed(-127)).expect("-127i8");
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_add(i8_type, input.clone(), positive.clone())
            .expect("first signed exact add"),
    );
    let parameters = BTreeSet::from([input_id]);
    assert_eq!(
        exact_integer_add_obligation(
            i8_type,
            first.clone(),
            positive,
            std::slice::from_ref(&first_definition),
            1,
            &parameters,
        ),
        Proposition::LessOrEqual(
            input,
            ScalarTerm::integer(i8_type, IntegerValue::Signed(-127)).expect("-127i8"),
        )
    );
    assert_eq!(
        exact_integer_add_obligation(
            i8_type,
            first,
            negative,
            std::slice::from_ref(&first_definition),
            1,
            &parameters,
        ),
        Proposition::Truth
    );

    let i128_type = IntegerType::new(IntegerSign::Signed, 128).expect("i128");
    let wide_input_id = ValueId::new(11).expect("wide input");
    let wide_input = ScalarTerm::value(wide_input_id, ScalarType::Integer(i128_type));
    let wide_first = ScalarTerm::value(
        ValueId::new(12).expect("wide first"),
        ScalarType::Integer(i128_type),
    );
    let wide_second = ScalarTerm::value(
        ValueId::new(13).expect("wide second"),
        ScalarType::Integer(i128_type),
    );
    let maximum =
        ScalarTerm::integer(i128_type, IntegerValue::Signed(i128::MAX)).expect("i128 maximum");
    let wide_first_definition = Proposition::Equal(
        wide_first.clone(),
        ScalarTerm::exact_integer_add(i128_type, wide_input, maximum.clone())
            .expect("wide first exact add"),
    );
    let wide_second_definition = Proposition::Equal(
        wide_second.clone(),
        ScalarTerm::exact_integer_add(i128_type, wide_first, maximum.clone())
            .expect("wide second exact add"),
    );
    assert_eq!(
        exact_integer_add_obligation(
            i128_type,
            wide_second,
            maximum,
            &[wide_first_definition, wide_second_definition],
            2,
            &BTreeSet::from([wide_input_id]),
        ),
        Proposition::Falsehood
    );
}

#[test]
fn reconstructs_a_finite_ordered_exact_subtract_chain_and_rejects_broken_definitions() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let input_id = ValueId::new(21).expect("input");
    let input = ScalarTerm::value(input_id, ScalarType::Integer(integer_type));
    let first = ScalarTerm::value(
        ValueId::new(22).expect("first"),
        ScalarType::Integer(integer_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(23).expect("second"),
        ScalarType::Integer(integer_type),
    );
    let one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).expect("1u8");
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_subtract(integer_type, input.clone(), one.clone())
            .expect("first exact subtract"),
    );
    let second_definition = Proposition::Equal(
        second.clone(),
        ScalarTerm::exact_integer_subtract(integer_type, first.clone(), one.clone())
            .expect("second exact subtract"),
    );
    let parameters = BTreeSet::from([input_id]);
    let expected = Proposition::LessOrEqual(
        ScalarTerm::integer(integer_type, IntegerValue::Unsigned(3)).expect("3u8"),
        input.clone(),
    );
    let reconstruct = |axioms: &[Proposition], parameters: &BTreeSet<ValueId>| {
        exact_integer_subtract_obligation(
            integer_type,
            second.clone(),
            one.clone(),
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
        expected
    );
    assert_ne!(
        reconstruct(std::slice::from_ref(&second_definition), &parameters),
        expected
    );
    assert_ne!(
        reconstruct(
            &[second_definition.clone(), first_definition.clone()],
            &parameters,
        ),
        expected
    );
    let reversed_second = match second_definition.clone() {
        Proposition::Equal(left, right) => Proposition::Equal(right, left),
        _ => unreachable!("exact-subtract definition is an equality"),
    };
    assert_ne!(
        reconstruct(&[first_definition.clone(), reversed_second], &parameters),
        expected
    );
    let redirected_second = Proposition::Equal(
        second.clone(),
        ScalarTerm::integer(integer_type, IntegerValue::Unsigned(2)).expect("2u8"),
    );
    assert_ne!(
        reconstruct(&[first_definition.clone(), redirected_second], &parameters),
        expected
    );
    let reversed_operand_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_subtract(integer_type, one.clone(), input)
            .expect("reversed exact subtract"),
    );
    assert_ne!(
        reconstruct(
            &[reversed_operand_definition, second_definition.clone()],
            &parameters,
        ),
        expected
    );
    let cyclic_first = Proposition::Equal(
        first,
        ScalarTerm::exact_integer_subtract(integer_type, second.clone(), one.clone())
            .expect("cyclic exact subtract"),
    );
    assert_ne!(
        reconstruct(&[cyclic_first, second_definition.clone()], &parameters),
        expected
    );
    assert_ne!(
        reconstruct(&[first_definition, second_definition], &BTreeSet::new()),
        expected
    );
}

#[test]
fn reconstructs_wide_signed_subtract_offsets_cancellation_and_magnitude_overflow() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let input_id = ValueId::new(31).expect("input");
    let input = ScalarTerm::value(input_id, ScalarType::Integer(i8_type));
    let first = ScalarTerm::value(
        ValueId::new(32).expect("first"),
        ScalarType::Integer(i8_type),
    );
    let positive = ScalarTerm::integer(i8_type, IntegerValue::Signed(127)).expect("127i8");
    let negative = ScalarTerm::integer(i8_type, IntegerValue::Signed(-127)).expect("-127i8");
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_subtract(i8_type, input.clone(), positive.clone())
            .expect("first signed exact subtract"),
    );
    let parameters = BTreeSet::from([input_id]);
    assert_eq!(
        exact_integer_subtract_obligation(
            i8_type,
            first.clone(),
            positive,
            std::slice::from_ref(&first_definition),
            1,
            &parameters,
        ),
        Proposition::LessOrEqual(
            ScalarTerm::integer(i8_type, IntegerValue::Signed(126)).expect("126i8"),
            input,
        )
    );
    assert_eq!(
        exact_integer_subtract_obligation(
            i8_type,
            first,
            negative,
            std::slice::from_ref(&first_definition),
            1,
            &parameters,
        ),
        Proposition::Truth
    );

    let i128_type = IntegerType::new(IntegerSign::Signed, 128).expect("i128");
    let wide_input_id = ValueId::new(41).expect("wide input");
    let wide_input = ScalarTerm::value(wide_input_id, ScalarType::Integer(i128_type));
    let wide_first = ScalarTerm::value(
        ValueId::new(42).expect("wide first"),
        ScalarType::Integer(i128_type),
    );
    let wide_second = ScalarTerm::value(
        ValueId::new(43).expect("wide second"),
        ScalarType::Integer(i128_type),
    );
    let maximum =
        ScalarTerm::integer(i128_type, IntegerValue::Signed(i128::MAX)).expect("i128 maximum");
    let wide_first_definition = Proposition::Equal(
        wide_first.clone(),
        ScalarTerm::exact_integer_subtract(i128_type, wide_input, maximum.clone())
            .expect("wide first exact subtract"),
    );
    let wide_second_definition = Proposition::Equal(
        wide_second.clone(),
        ScalarTerm::exact_integer_subtract(i128_type, wide_first, maximum.clone())
            .expect("wide second exact subtract"),
    );
    assert_eq!(
        exact_integer_subtract_obligation(
            i128_type,
            wide_second,
            maximum,
            &[wide_first_definition, wide_second_definition],
            2,
            &BTreeSet::from([wide_input_id]),
        ),
        Proposition::Falsehood
    );
}

#[test]
fn reconstructs_mixed_exact_add_subtract_offsets_and_rejects_broken_chains() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let input_id = ValueId::new(51).expect("input");
    let input = ScalarTerm::value(input_id, ScalarType::Integer(integer_type));
    let first = ScalarTerm::value(
        ValueId::new(52).expect("first"),
        ScalarType::Integer(integer_type),
    );
    let second = ScalarTerm::value(
        ValueId::new(53).expect("second"),
        ScalarType::Integer(integer_type),
    );
    let five = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(5)).expect("5u8");
    let three = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(3)).expect("3u8");
    let two = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(2)).expect("2u8");
    let first_definition = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_add(integer_type, input.clone(), five).expect("input + 5u8"),
    );
    let second_definition = Proposition::Equal(
        second.clone(),
        ScalarTerm::exact_integer_subtract(integer_type, first.clone(), three)
            .expect("first - 3u8"),
    );
    let parameters = BTreeSet::from([input_id]);
    assert_eq!(
        exact_integer_subtract_obligation(
            integer_type,
            first.clone(),
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(3)).expect("3u8"),
            std::slice::from_ref(&first_definition),
            1,
            &parameters,
        ),
        Proposition::LessOrEqual(
            input.clone(),
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(253)).expect("253u8"),
        ),
        "the mixed second prefix is reconstructed from the direct root"
    );
    assert_eq!(
        exact_integer_subtract_obligation(
            integer_type,
            first.clone(),
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(5)).expect("5u8"),
            std::slice::from_ref(&first_definition),
            1,
            &parameters,
        ),
        Proposition::Truth,
        "cancellation is total only after the earlier prefix keeps its own proof"
    );
    let expected = Proposition::LessOrEqual(
        input.clone(),
        ScalarTerm::integer(integer_type, IntegerValue::Unsigned(251)).expect("251u8"),
    );
    let reconstruct = |axioms: &[Proposition], parameters: &BTreeSet<ValueId>| {
        exact_integer_add_obligation(
            integer_type,
            second.clone(),
            two.clone(),
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
        expected
    );
    assert_ne!(
        reconstruct(
            &[second_definition.clone(), first_definition.clone()],
            &parameters,
        ),
        expected
    );
    let reversed_second = match second_definition.clone() {
        Proposition::Equal(left, right) => Proposition::Equal(right, left),
        _ => unreachable!("mixed definition is an equality"),
    };
    assert_ne!(
        reconstruct(&[first_definition.clone(), reversed_second], &parameters),
        expected
    );
    let redirected_second = Proposition::Equal(
        second.clone(),
        ScalarTerm::integer(integer_type, IntegerValue::Unsigned(2)).expect("2u8"),
    );
    assert_ne!(
        reconstruct(&[first_definition.clone(), redirected_second], &parameters),
        expected
    );
    let right_associated_first = Proposition::Equal(
        first.clone(),
        ScalarTerm::exact_integer_add(
            integer_type,
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(5)).expect("5u8"),
            input.clone(),
        )
        .expect("5u8 + input"),
    );
    assert_ne!(
        reconstruct(
            &[right_associated_first, second_definition.clone()],
            &parameters,
        ),
        expected
    );
    let cyclic_first = Proposition::Equal(
        first,
        ScalarTerm::exact_integer_add(integer_type, second.clone(), two.clone())
            .expect("second + 2u8"),
    );
    assert_ne!(
        reconstruct(&[cyclic_first, second_definition.clone()], &parameters),
        expected
    );
    assert_ne!(
        reconstruct(&[first_definition, second_definition], &BTreeSet::new()),
        expected
    );

    let signed_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_input_id = ValueId::new(61).expect("signed input");
    let signed_input = ScalarTerm::value(signed_input_id, ScalarType::Integer(signed_type));
    let signed_first = ScalarTerm::value(
        ValueId::new(62).expect("signed first"),
        ScalarType::Integer(signed_type),
    );
    let signed_definition = Proposition::Equal(
        signed_first.clone(),
        ScalarTerm::exact_integer_subtract(
            signed_type,
            signed_input.clone(),
            ScalarTerm::integer(signed_type, IntegerValue::Signed(-3)).expect("-3i8"),
        )
        .expect("signed input - -3i8"),
    );
    assert_eq!(
        exact_integer_add_obligation(
            signed_type,
            signed_first,
            ScalarTerm::integer(signed_type, IntegerValue::Signed(-5)).expect("-5i8"),
            std::slice::from_ref(&signed_definition),
            1,
            &BTreeSet::from([signed_input_id]),
        ),
        Proposition::LessOrEqual(
            ScalarTerm::integer(signed_type, IntegerValue::Signed(-126)).expect("-126i8"),
            signed_input,
        )
    );

    let overflow_input_id = ValueId::new(71).expect("overflow input");
    let overflow_input = ScalarTerm::value(overflow_input_id, ScalarType::Integer(integer_type));
    let overflow_first = ScalarTerm::value(
        ValueId::new(72).expect("overflow first"),
        ScalarType::Integer(integer_type),
    );
    let overflow_second = ScalarTerm::value(
        ValueId::new(73).expect("overflow second"),
        ScalarType::Integer(integer_type),
    );
    let subtract_zero = Proposition::Equal(
        overflow_first.clone(),
        ScalarTerm::exact_integer_subtract(
            integer_type,
            overflow_input,
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(0)).expect("0u8"),
        )
        .expect("input - 0u8"),
    );
    let add_maximum = Proposition::Equal(
        overflow_second.clone(),
        ScalarTerm::exact_integer_add(
            integer_type,
            overflow_first,
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(255)).expect("255u8"),
        )
        .expect("first + 255u8"),
    );
    assert_eq!(
        exact_integer_add_obligation(
            integer_type,
            overflow_second,
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).expect("1u8"),
            &[subtract_zero, add_maximum],
            2,
            &BTreeSet::from([overflow_input_id]),
        ),
        Proposition::Falsehood
    );
}

#[test]
fn reconstructs_signed_nonnegative_joint_exact_add_bounds() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = ScalarTerm::value(
        ValueId::new(4).expect("left"),
        ScalarType::Integer(integer_type),
    );
    let right = ScalarTerm::value(
        ValueId::new(5).expect("right"),
        ScalarType::Integer(integer_type),
    );
    let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).expect("0i8");
    let maximum = ScalarTerm::integer(integer_type, IntegerValue::Signed(127)).expect("127i8");
    let remainder = ScalarTerm::exact_integer_subtract(integer_type, maximum, right.clone())
        .expect("127 - right");
    let nonnegative = Proposition::LessOrEqual(zero, right.clone());
    let bound = Proposition::LessOrEqual(left.clone(), remainder);
    let axioms = vec![nonnegative.clone(), bound.clone()];
    assert_eq!(
        exact_integer_add_obligation(
            integer_type,
            left,
            right.clone(),
            &axioms,
            0,
            &BTreeSet::new(),
        ),
        canonical_conjunction(vec![nonnegative.clone(), bound])
    );
    assert_eq!(
        exact_integer_subtract_obligation(
            integer_type,
            ScalarTerm::integer(integer_type, IntegerValue::Signed(127)).expect("127i8"),
            right,
            std::slice::from_ref(&nonnegative),
            0,
            &BTreeSet::new(),
        ),
        nonnegative
    );
}

#[test]
fn reconstructs_signed_nonpositive_joint_exact_add_bounds() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = ScalarTerm::value(
        ValueId::new(6).expect("left"),
        ScalarType::Integer(integer_type),
    );
    let right = ScalarTerm::value(
        ValueId::new(7).expect("right"),
        ScalarType::Integer(integer_type),
    );
    let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).expect("0i8");
    let minimum = ScalarTerm::integer(integer_type, IntegerValue::Signed(-128)).expect("-128i8");
    let remainder = ScalarTerm::exact_integer_subtract(integer_type, minimum, right.clone())
        .expect("-128 - right");
    let nonpositive = Proposition::LessOrEqual(right.clone(), zero);
    let bound = Proposition::LessOrEqual(remainder, left.clone());
    let axioms = vec![nonpositive.clone(), bound.clone()];
    assert_eq!(
        exact_integer_add_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            &axioms,
            0,
            &BTreeSet::new(),
        ),
        canonical_conjunction(vec![nonpositive.clone(), bound])
    );
    assert_eq!(
        exact_integer_subtract_obligation(
            integer_type,
            ScalarTerm::integer(integer_type, IntegerValue::Signed(-128)).expect("-128i8"),
            right.clone(),
            std::slice::from_ref(&nonpositive),
            0,
            &BTreeSet::new(),
        ),
        nonpositive
    );
    assert_eq!(
        exact_integer_subtract_obligation(
            integer_type,
            ScalarTerm::integer(integer_type, IntegerValue::Signed(-128)).expect("-128i8"),
            right,
            &[axioms[1].clone()],
            0,
            &BTreeSet::new(),
        ),
        Proposition::Falsehood
    );
}

#[test]
fn reconstructs_unsigned_joint_exact_subtract_bounds() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::value(
        ValueId::new(8).expect("left"),
        ScalarType::Integer(integer_type),
    );
    let right = ScalarTerm::value(
        ValueId::new(9).expect("right"),
        ScalarType::Integer(integer_type),
    );
    let bound = Proposition::LessOrEqual(right.clone(), left.clone());
    assert_eq!(
        exact_integer_subtract_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            std::slice::from_ref(&bound),
            0,
            &BTreeSet::new(),
        ),
        bound.clone()
    );
    assert_eq!(
        exact_integer_subtract_obligation(integer_type, left, right, &[], 0, &BTreeSet::new(),),
        Proposition::Falsehood
    );
}

#[test]
fn reconstructs_signed_nonnegative_joint_exact_subtract_bounds() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = ScalarTerm::value(
        ValueId::new(10).expect("left"),
        ScalarType::Integer(integer_type),
    );
    let right = ScalarTerm::value(
        ValueId::new(11).expect("right"),
        ScalarType::Integer(integer_type),
    );
    let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).expect("0i8");
    let minimum = ScalarTerm::integer(integer_type, IntegerValue::Signed(-128)).expect("-128i8");
    let lower =
        ScalarTerm::exact_integer_add(integer_type, minimum, right.clone()).expect("-128 + right");
    let nonnegative = Proposition::LessOrEqual(zero, right.clone());
    let bound = Proposition::LessOrEqual(lower, left.clone());
    let axioms = vec![nonnegative.clone(), bound.clone()];
    assert_eq!(
        exact_integer_subtract_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            &axioms,
            0,
            &BTreeSet::new(),
        ),
        canonical_conjunction(vec![nonnegative.clone(), bound.clone()])
    );
    assert_eq!(
        exact_integer_subtract_obligation(
            integer_type,
            left,
            right,
            &axioms[1..],
            0,
            &BTreeSet::new(),
        ),
        Proposition::Falsehood
    );
}

#[test]
fn reconstructs_signed_nonpositive_joint_exact_subtract_bounds() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = ScalarTerm::value(
        ValueId::new(12).expect("left"),
        ScalarType::Integer(integer_type),
    );
    let right = ScalarTerm::value(
        ValueId::new(13).expect("right"),
        ScalarType::Integer(integer_type),
    );
    let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).expect("0i8");
    let maximum = ScalarTerm::integer(integer_type, IntegerValue::Signed(127)).expect("127i8");
    let upper =
        ScalarTerm::exact_integer_add(integer_type, maximum, right.clone()).expect("127 + right");
    let nonpositive = Proposition::LessOrEqual(right.clone(), zero);
    let bound = Proposition::LessOrEqual(left.clone(), upper);
    let axioms = vec![nonpositive.clone(), bound.clone()];
    assert_eq!(
        exact_integer_subtract_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            &axioms,
            0,
            &BTreeSet::new(),
        ),
        canonical_conjunction(vec![nonpositive.clone(), bound.clone()])
    );
    assert_eq!(
        exact_integer_subtract_obligation(
            integer_type,
            left,
            right,
            &axioms[1..],
            0,
            &BTreeSet::new(),
        ),
        Proposition::Falsehood
    );
}

#[test]
fn exact_subtract_reconstructs_carrier_tight_known_right_bounds() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let unsigned_left = ScalarTerm::value(
        ValueId::new(1).expect("value"),
        ScalarType::Integer(u8_type),
    );
    let unsigned_five = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(5)).expect("5u8");
    assert_eq!(
        exact_integer_subtract_obligation(
            u8_type,
            unsigned_left.clone(),
            unsigned_five.clone(),
            &[],
            0,
            &BTreeSet::new(),
        ),
        Proposition::LessOrEqual(unsigned_five, unsigned_left)
    );

    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_left = ScalarTerm::value(
        ValueId::new(2).expect("value"),
        ScalarType::Integer(i8_type),
    );
    let positive = ScalarTerm::integer(i8_type, IntegerValue::Signed(8)).expect("8i8");
    let lower = ScalarTerm::integer(i8_type, IntegerValue::Signed(-120)).expect("-120i8");
    assert_eq!(
        exact_integer_subtract_obligation(
            i8_type,
            signed_left.clone(),
            positive,
            &[],
            0,
            &BTreeSet::new(),
        ),
        Proposition::LessOrEqual(lower, signed_left.clone())
    );

    let negative = ScalarTerm::integer(i8_type, IntegerValue::Signed(-7)).expect("-7i8");
    let upper = ScalarTerm::integer(i8_type, IntegerValue::Signed(120)).expect("120i8");
    assert_eq!(
        exact_integer_subtract_obligation(
            i8_type,
            signed_left.clone(),
            negative,
            &[],
            0,
            &BTreeSet::new(),
        ),
        Proposition::LessOrEqual(signed_left, upper)
    );
}

#[test]
fn exact_subtract_fails_closed_without_a_known_right_operand() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let value = |id| {
        ScalarTerm::value(
            ValueId::new(id).expect("value"),
            ScalarType::Integer(integer_type),
        )
    };
    assert_eq!(
        exact_integer_subtract_obligation(
            integer_type,
            value(1),
            value(2),
            &[],
            0,
            &BTreeSet::new(),
        ),
        Proposition::Falsehood
    );

    let four = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(4)).expect("4u8");
    let five = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(5)).expect("5u8");
    assert_eq!(
        exact_integer_subtract_obligation(integer_type, four, five, &[], 0, &BTreeSet::new(),),
        Proposition::Falsehood
    );
}

#[test]
fn reconstructs_known_unsigned_minuend_bounds() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let right = ScalarTerm::value(
        ValueId::new(3).expect("right"),
        ScalarType::Integer(integer_type),
    );
    let maximum = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(255)).expect("255u8");
    assert_eq!(
        exact_integer_subtract_obligation(
            integer_type,
            maximum.clone(),
            right.clone(),
            &[],
            0,
            &BTreeSet::new(),
        ),
        Proposition::Truth
    );
}
