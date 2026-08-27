use super::super::*;
use psi_core::{IntegerType, ScalarType, ValueId};

#[test]
fn exact_multiply_fails_closed_without_a_known_factor() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let value = |id| {
        ScalarTerm::value(
            ValueId::new(id).expect("value"),
            ScalarType::Integer(integer_type),
        )
    };
    assert_eq!(
        exact_integer_multiply_obligation(integer_type, value(1), value(2), &[],),
        Proposition::Falsehood
    );
    let fifty_one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(51)).expect("51u8");
    let five = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(5)).expect("5u8");
    assert_eq!(
        exact_integer_multiply_obligation(integer_type, fifty_one, five, &[],),
        Proposition::Truth
    );
}

#[test]
fn reconstructs_unsigned_joint_exact_multiply_bounds() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::value(
        ValueId::new(14).expect("left"),
        ScalarType::Integer(integer_type),
    );
    let right = ScalarTerm::value(
        ValueId::new(15).expect("right"),
        ScalarType::Integer(integer_type),
    );
    let one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).expect("1u8");
    let maximum = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(255)).expect("255u8");
    let upper = ScalarTerm::exact_integer_divide(integer_type, maximum, right.clone())
        .expect("255 / right");
    let positive = Proposition::LessOrEqual(one, right.clone());
    let bound = Proposition::LessOrEqual(left.clone(), upper);
    let axioms = vec![positive.clone(), bound.clone()];
    assert_eq!(
        exact_integer_multiply_obligation(integer_type, left.clone(), right.clone(), &axioms,),
        canonical_conjunction(vec![positive.clone(), bound.clone()])
    );
    assert_eq!(
        exact_integer_multiply_obligation(integer_type, left, right, &axioms[1..],),
        Proposition::Falsehood
    );
}

#[test]
fn reconstructs_signed_positive_joint_exact_multiply_bounds() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = ScalarTerm::value(
        ValueId::new(16).expect("left"),
        ScalarType::Integer(integer_type),
    );
    let right = ScalarTerm::value(
        ValueId::new(17).expect("right"),
        ScalarType::Integer(integer_type),
    );
    let one = ScalarTerm::integer(integer_type, IntegerValue::Signed(1)).expect("1i8");
    let minimum = ScalarTerm::integer(integer_type, IntegerValue::Signed(-128)).expect("-128i8");
    let maximum = ScalarTerm::integer(integer_type, IntegerValue::Signed(127)).expect("127i8");
    let lower = ScalarTerm::exact_integer_divide(integer_type, minimum, right.clone())
        .expect("-128 / right");
    let upper = ScalarTerm::exact_integer_divide(integer_type, maximum, right.clone())
        .expect("127 / right");
    let positive = Proposition::LessOrEqual(one, right.clone());
    let lower_bound = Proposition::LessOrEqual(lower, left.clone());
    let upper_bound = Proposition::LessOrEqual(left.clone(), upper);
    let axioms = vec![positive.clone(), lower_bound.clone(), upper_bound.clone()];
    assert_eq!(
        exact_integer_multiply_obligation(integer_type, left.clone(), right.clone(), &axioms,),
        canonical_conjunction(vec![
            positive.clone(),
            lower_bound.clone(),
            upper_bound.clone(),
        ])
    );
    assert_eq!(
        exact_integer_multiply_obligation(integer_type, left, right, &axioms[..2],),
        Proposition::Falsehood
    );
}

#[test]
fn reconstructs_signed_negative_joint_exact_multiply_bounds() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = ScalarTerm::value(
        ValueId::new(18).expect("left"),
        ScalarType::Integer(integer_type),
    );
    let right = ScalarTerm::value(
        ValueId::new(19).expect("right"),
        ScalarType::Integer(integer_type),
    );
    let negative_two = ScalarTerm::integer(integer_type, IntegerValue::Signed(-2)).expect("-2i8");
    let minimum = ScalarTerm::integer(integer_type, IntegerValue::Signed(-128)).expect("-128i8");
    let maximum = ScalarTerm::integer(integer_type, IntegerValue::Signed(127)).expect("127i8");
    let lower = ScalarTerm::exact_integer_divide(integer_type, maximum, right.clone())
        .expect("127 / right");
    let upper = ScalarTerm::exact_integer_divide(integer_type, minimum, right.clone())
        .expect("-128 / right");
    let negative = Proposition::LessOrEqual(right.clone(), negative_two);
    let lower_bound = Proposition::LessOrEqual(lower, left.clone());
    let upper_bound = Proposition::LessOrEqual(left.clone(), upper);
    let axioms = vec![negative.clone(), lower_bound.clone(), upper_bound.clone()];
    assert_eq!(
        exact_integer_multiply_obligation(integer_type, left.clone(), right.clone(), &axioms,),
        canonical_conjunction(vec![
            negative.clone(),
            lower_bound.clone(),
            upper_bound.clone(),
        ])
    );
    assert_eq!(
        exact_integer_multiply_obligation(integer_type, left, right, &axioms[..2],),
        Proposition::Falsehood
    );
}

#[test]
fn exact_divide_reconstructs_known_divisor_safety() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let value = ScalarTerm::value(
        ValueId::new(1).expect("value"),
        ScalarType::Integer(u8_type),
    );
    let five = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(5)).expect("5u8");
    assert_eq!(
        exact_integer_divide_obligation(u8_type, value.clone(), five, &[]),
        Proposition::Truth
    );
    let zero = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(0)).expect("0u8");
    assert_eq!(
        exact_integer_divide_obligation(u8_type, value, zero, &[]),
        Proposition::Falsehood
    );

    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let value = ScalarTerm::value(
        ValueId::new(2).expect("value"),
        ScalarType::Integer(i8_type),
    );
    let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).expect("-1i8");
    let minimum_plus_one =
        ScalarTerm::integer(i8_type, IntegerValue::Signed(-127)).expect("-127i8");
    assert_eq!(
        exact_integer_divide_obligation(i8_type, value.clone(), negative_one, &[]),
        Proposition::LessOrEqual(minimum_plus_one, value)
    );
    let unknown = ScalarTerm::value(
        ValueId::new(3).expect("divisor"),
        ScalarType::Integer(i8_type),
    );
    let one = ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).unwrap();
    assert_eq!(
        exact_integer_divide_obligation(i8_type, one.clone(), unknown.clone(), &[]),
        Proposition::LessOrEqual(one, unknown)
    );
}

#[test]
fn exact_remainder_reconstructs_known_divisor_safety() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let value = ScalarTerm::value(
        ValueId::new(1).expect("value"),
        ScalarType::Integer(u8_type),
    );
    let five = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(5)).expect("5u8");
    assert_eq!(
        exact_integer_remainder_obligation(u8_type, value.clone(), five, &[]),
        Proposition::Truth
    );
    let zero = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(0)).expect("0u8");
    assert_eq!(
        exact_integer_remainder_obligation(u8_type, value, zero, &[]),
        Proposition::Falsehood
    );

    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let value = ScalarTerm::value(
        ValueId::new(2).expect("value"),
        ScalarType::Integer(i8_type),
    );
    let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).expect("-1i8");
    let minimum_plus_one =
        ScalarTerm::integer(i8_type, IntegerValue::Signed(-127)).expect("-127i8");
    assert_eq!(
        exact_integer_remainder_obligation(i8_type, value.clone(), negative_one, &[]),
        Proposition::LessOrEqual(minimum_plus_one, value)
    );
    let unknown = ScalarTerm::value(
        ValueId::new(3).expect("divisor"),
        ScalarType::Integer(i8_type),
    );
    let one = ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).unwrap();
    assert_eq!(
        exact_integer_remainder_obligation(i8_type, one.clone(), unknown.clone(), &[]),
        Proposition::LessOrEqual(one, unknown)
    );
}

#[test]
fn mixed_exact_divide_remainder_chain_reconstructs_each_safe_divisor_independently() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let root = ScalarTerm::value(ValueId::new(1).expect("root"), ScalarType::Integer(u8_type));
    let two = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(2)).expect("2u8");
    let three = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(3)).expect("3u8");
    let inner = ScalarTerm::exact_integer_divide(u8_type, root, two.clone()).expect("root / 2u8");
    assert_eq!(
        exact_integer_remainder_obligation(u8_type, inner.clone(), three.clone(), &[]),
        Proposition::Truth
    );
    let middle =
        ScalarTerm::exact_integer_remainder(u8_type, inner, three).expect("(root / 2u8) % 3u8");
    assert_eq!(
        exact_integer_divide_obligation(u8_type, middle, two, &[]),
        Proposition::Truth
    );
}

#[test]
fn runtime_divisor_chains_reconstruct_each_parameter_guard_independently() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let root = ScalarTerm::value(
        ValueId::new(21).expect("root"),
        ScalarType::Integer(u8_type),
    );
    let positive_divisor = ScalarTerm::value(
        ValueId::new(22).expect("positive divisor"),
        ScalarType::Integer(u8_type),
    );
    let two = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(2)).expect("2u8");
    let positive_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).expect("1u8"),
        positive_divisor.clone(),
    );
    let first = ScalarTerm::exact_integer_divide(u8_type, root, positive_divisor.clone())
        .expect("root / divisor");
    assert_eq!(
        exact_integer_divide_obligation(
            u8_type,
            first.clone(),
            positive_divisor,
            std::slice::from_ref(&positive_bound),
        ),
        positive_bound,
        "the nested dividend supplies no authority beyond the direct divisor guard",
    );
    assert_eq!(
        exact_integer_remainder_obligation(u8_type, first, two, &[]),
        Proposition::Truth,
    );

    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_root = ScalarTerm::value(
        ValueId::new(23).expect("signed root"),
        ScalarType::Integer(i8_type),
    );
    let negative_divisor = ScalarTerm::value(
        ValueId::new(24).expect("negative divisor"),
        ScalarType::Integer(i8_type),
    );
    let negative_bound = Proposition::LessOrEqual(
        negative_divisor.clone(),
        ScalarTerm::integer(i8_type, IntegerValue::Signed(-2)).expect("-2i8"),
    );
    let signed_first =
        ScalarTerm::exact_integer_remainder(i8_type, signed_root.clone(), negative_divisor.clone())
            .expect("signed root % negative divisor");
    assert_eq!(
        exact_integer_remainder_obligation(
            i8_type,
            signed_first,
            negative_divisor.clone(),
            std::slice::from_ref(&negative_bound),
        ),
        negative_bound,
    );

    let negative_one_bound = Proposition::LessOrEqual(
        negative_divisor.clone(),
        ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).expect("-1i8"),
    );
    let minimum_plus_one_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(i8_type, IntegerValue::Signed(-127)).expect("-127i8"),
        signed_root.clone(),
    );
    assert_eq!(
        exact_integer_divide_obligation(
            i8_type,
            signed_root.clone(),
            negative_divisor.clone(),
            &[negative_one_bound.clone(), minimum_plus_one_bound.clone()],
        ),
        canonical_conjunction(vec![
            negative_one_bound.clone(),
            minimum_plus_one_bound.clone(),
        ]),
        "the joint -1 exception remains valid for the independently bounded direct root",
    );
    let computed_dividend = ScalarTerm::exact_integer_divide(
        i8_type,
        signed_root,
        ScalarTerm::integer(i8_type, IntegerValue::Signed(2)).expect("2i8"),
    )
    .expect("computed dividend");
    let fallback_positive_requirement = Proposition::LessOrEqual(
        ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
        negative_divisor.clone(),
    );
    assert_eq!(
        exact_integer_divide_obligation(
            i8_type,
            computed_dividend,
            negative_divisor,
            &[negative_one_bound, minimum_plus_one_bound],
        ),
        fallback_positive_requirement,
        "a direct-root bound is never imported as a computed-prefix dividend bound",
    );
}
