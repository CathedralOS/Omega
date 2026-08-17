use super::super::*;
use psi_core::{IntegerType, ScalarType, ValueId};

#[test]
fn wrapping_divide_reconstructs_known_nonzero_divisor_safety() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let value = ScalarTerm::value(
        ValueId::new(1).expect("value"),
        ScalarType::Integer(i8_type),
    );
    let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
    assert_eq!(
        wrapping_integer_divide_obligation(i8_type, value.clone(), negative_one, &[]),
        Proposition::Truth
    );
    let zero = ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).unwrap();
    assert_eq!(
        wrapping_integer_divide_obligation(i8_type, value, zero, &[]),
        Proposition::Falsehood
    );
    let minimum = ScalarTerm::integer(i8_type, IntegerValue::Signed(-128)).unwrap();
    let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
    assert_eq!(
        wrapping_integer_divide_obligation(i8_type, minimum, negative_one, &[]),
        Proposition::Truth
    );
    let unknown = ScalarTerm::value(
        ValueId::new(2).expect("divisor"),
        ScalarType::Integer(i8_type),
    );
    let one = ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).unwrap();
    assert_eq!(
        wrapping_integer_divide_obligation(i8_type, one.clone(), unknown.clone(), &[]),
        Proposition::LessOrEqual(one, unknown)
    );
}

#[test]
fn wrapping_remainder_reconstructs_known_nonzero_divisor_safety() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let value = ScalarTerm::value(
        ValueId::new(1).expect("value"),
        ScalarType::Integer(i8_type),
    );
    let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
    assert_eq!(
        wrapping_integer_remainder_obligation(i8_type, value.clone(), negative_one, &[]),
        Proposition::Truth
    );
    let zero = ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).unwrap();
    assert_eq!(
        wrapping_integer_remainder_obligation(i8_type, value, zero, &[]),
        Proposition::Falsehood
    );
    let minimum = ScalarTerm::integer(i8_type, IntegerValue::Signed(-128)).unwrap();
    let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
    assert_eq!(
        wrapping_integer_remainder_obligation(i8_type, minimum, negative_one, &[]),
        Proposition::Truth
    );
}

#[test]
fn saturating_divide_reconstructs_known_nonzero_divisor_safety() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let value = ScalarTerm::value(
        ValueId::new(1).expect("value"),
        ScalarType::Integer(i8_type),
    );
    let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
    assert_eq!(
        saturating_integer_divide_obligation(i8_type, value.clone(), negative_one, &[]),
        Proposition::Truth
    );
    let zero = ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).unwrap();
    assert_eq!(
        saturating_integer_divide_obligation(i8_type, value, zero, &[]),
        Proposition::Falsehood
    );
    let minimum = ScalarTerm::integer(i8_type, IntegerValue::Signed(-128)).unwrap();
    let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
    assert_eq!(
        saturating_integer_divide_obligation(i8_type, minimum, negative_one, &[]),
        Proposition::Truth
    );
}

#[test]
fn saturating_remainder_reconstructs_known_nonzero_divisor_safety() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let value = ScalarTerm::value(
        ValueId::new(1).expect("value"),
        ScalarType::Integer(i8_type),
    );
    let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
    assert_eq!(
        saturating_integer_remainder_obligation(i8_type, value.clone(), negative_one, &[]),
        Proposition::Truth
    );
    let zero = ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).unwrap();
    assert_eq!(
        saturating_integer_remainder_obligation(i8_type, value, zero, &[]),
        Proposition::Falsehood
    );
    let minimum = ScalarTerm::integer(i8_type, IntegerValue::Signed(-128)).unwrap();
    let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
    assert_eq!(
        saturating_integer_remainder_obligation(i8_type, minimum, negative_one, &[]),
        Proposition::Truth
    );
}

#[test]
fn runtime_divisor_bounds_reconstruct_for_every_policy() {
    type Reconstruct = fn(IntegerType, ScalarTerm, ScalarTerm, &[Proposition]) -> Proposition;
    let reconstructors: [Reconstruct; 6] = [
        exact_integer_divide_obligation,
        exact_integer_remainder_obligation,
        wrapping_integer_divide_obligation,
        wrapping_integer_remainder_obligation,
        saturating_integer_divide_obligation,
        saturating_integer_remainder_obligation,
    ];
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::value(
        ValueId::new(10).expect("left"),
        ScalarType::Integer(integer_type),
    );
    let right = ScalarTerm::value(
        ValueId::new(11).expect("right"),
        ScalarType::Integer(integer_type),
    );
    let one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).expect("one");
    let expected = Proposition::LessOrEqual(one, right.clone());

    for &reconstruct in &reconstructors {
        assert_eq!(
            reconstruct(integer_type, left.clone(), right.clone(), &[],),
            expected
        );
    }

    let signed_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = ScalarTerm::value(
        ValueId::new(14).expect("left"),
        ScalarType::Integer(signed_type),
    );
    let right = ScalarTerm::value(
        ValueId::new(15).expect("right"),
        ScalarType::Integer(signed_type),
    );
    let negative_two = ScalarTerm::integer(signed_type, IntegerValue::Signed(-2)).expect("-2i8");
    let negative_bound = Proposition::LessOrEqual(right.clone(), negative_two);
    for &reconstruct in &reconstructors {
        assert_eq!(
            reconstruct(
                signed_type,
                left.clone(),
                right.clone(),
                std::slice::from_ref(&negative_bound),
            ),
            negative_bound.clone()
        );
    }

    let minimum_plus_one =
        ScalarTerm::integer(signed_type, IntegerValue::Signed(-127)).expect("-127i8");
    let negative_one = ScalarTerm::integer(signed_type, IntegerValue::Signed(-1)).expect("-1i8");
    let negative_one_bound = Proposition::LessOrEqual(right.clone(), negative_one);
    let dividend_bound = Proposition::LessOrEqual(minimum_plus_one, left.clone());
    let exact_expected =
        canonical_conjunction(vec![negative_one_bound.clone(), dividend_bound.clone()]);
    for (index, &reconstruct) in reconstructors.iter().enumerate() {
        let axioms = if index < 2 {
            vec![negative_one_bound.clone(), dividend_bound.clone()]
        } else {
            vec![negative_one_bound.clone()]
        };
        assert_eq!(
            reconstruct(signed_type, left.clone(), right.clone(), &axioms,),
            if index < 2 {
                exact_expected.clone()
            } else {
                negative_one_bound.clone()
            }
        );
    }

    let signed_one_bit = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
    let left = ScalarTerm::value(
        ValueId::new(12).expect("left"),
        ScalarType::Integer(signed_one_bit),
    );
    let right = ScalarTerm::value(
        ValueId::new(13).expect("right"),
        ScalarType::Integer(signed_one_bit),
    );
    assert_eq!(
        exact_integer_divide_obligation(signed_one_bit, left, right, &[],),
        Proposition::Falsehood
    );
}
