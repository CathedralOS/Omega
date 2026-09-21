//! Tests for integer types, scalar terms and propositions.

use super::{
    IntegerCarrier, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, ProofTerm,
    ProofTermField, Proposition, PropositionContext, PropositionError, ScalarTerm, ScalarType,
};
use crate::ValueId;

#[test]
fn integer_widening_requires_range_containment_and_preserves_closed_values() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");

    assert!(u8_type.can_widen_to(u16_type));
    assert!(i8_type.can_widen_to(i64_type));
    assert!(u8_type.can_widen_to(i64_type));
    assert!(!u16_type.can_widen_to(u8_type));
    assert!(!u8_type.can_widen_to(u8_type));
    assert!(!i8_type.can_widen_to(u16_type));

    let widened = ScalarTerm::integer_widen(
        i8_type,
        i64_type,
        ScalarTerm::integer(i8_type, IntegerValue::Signed(-128)).expect("i8 literal"),
    )
    .expect("signed widening");
    assert_eq!(widened.scalar_type(), ScalarType::Integer(i64_type));
    assert_eq!(
        widened.integer_value(),
        Some((i64_type, IntegerValue::Signed(-128)))
    );

    let cross_signedness = ScalarTerm::integer_widen(
        u8_type,
        i64_type,
        ScalarTerm::integer(u8_type, IntegerValue::Unsigned(255)).expect("u8 literal"),
    )
    .expect("the complete u8 range fits i64");
    assert_eq!(
        cross_signedness.integer_value(),
        Some((i64_type, IntegerValue::Signed(255)))
    );

    let narrowing = ScalarTerm::integer_widen(
        u16_type,
        u8_type,
        ScalarTerm::integer(u16_type, IntegerValue::Unsigned(1)).expect("u16 literal"),
    );
    assert!(matches!(
        narrowing,
        Err(PropositionError::IntegerWidenTypeMismatch { .. })
    ));
}

#[test]
fn integer_literals_are_checked_against_their_terminal_type() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 type");
    assert!(ScalarTerm::integer(i8_type, IntegerValue::Signed(127)).is_ok());
    assert!(ScalarTerm::integer(i8_type, IntegerValue::Signed(128)).is_err());

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 type");
    assert!(ScalarTerm::integer(u8_type, IntegerValue::Unsigned(255)).is_ok());
    assert!(ScalarTerm::integer(u8_type, IntegerValue::Unsigned(256)).is_err());
}

#[test]
fn address_carriers_remain_distinct_from_same_width_unsigned_integers() {
    let address = IntegerType::address(64).expect("addr");
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");

    assert_eq!(address.carrier(), IntegerCarrier::Address);
    assert!(address.is_address());
    assert_eq!(address.sign(), IntegerSign::Unsigned);
    assert_eq!(address.bits(), 64);
    assert_ne!(address, u64_type);
    assert!(!address.can_widen_to(u64_type));
    assert!(!u64_type.can_widen_to(address));
    assert!(address.admits(IntegerValue::Unsigned(u64::MAX.into())));
}

#[test]
fn ordered_comparisons_require_one_exact_integer_type() {
    let boolean = ScalarTerm::boolean(false);
    assert_eq!(
        Proposition::LessThan(boolean.clone(), boolean)
            .validate()
            .expect_err("booleans are unordered"),
        PropositionError::OrderedComparisonRequiresIntegers(ScalarType::Boolean)
    );
}

#[test]
fn boolean_not_is_typed_and_reduces_closed_terms() {
    let negated = ScalarTerm::boolean_not(ScalarTerm::boolean(false)).unwrap();
    assert_eq!(negated.scalar_type(), ScalarType::Boolean);
    assert_eq!(negated.boolean_value(), Some(true));
    assert_eq!(
        ScalarTerm::boolean_not(
            ScalarTerm::integer(
                IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                IntegerValue::Unsigned(1),
            )
            .unwrap(),
        ),
        Err(PropositionError::BooleanNotTypeMismatch(
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"))
        ))
    );
}

#[test]
fn boolean_equality_is_typed_and_reduces_closed_terms() {
    let equal =
        ScalarTerm::boolean_equal(ScalarTerm::boolean(false), ScalarTerm::boolean(false)).unwrap();
    let unequal =
        ScalarTerm::boolean_equal(ScalarTerm::boolean(false), ScalarTerm::boolean(true)).unwrap();
    assert_eq!(equal.scalar_type(), ScalarType::Boolean);
    assert_eq!(equal.boolean_value(), Some(true));
    assert_eq!(unequal.boolean_value(), Some(false));

    let integer = ScalarTerm::integer(
        IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
        IntegerValue::Unsigned(1),
    )
    .unwrap();
    assert!(matches!(
        ScalarTerm::boolean_equal(ScalarTerm::boolean(true), integer),
        Err(PropositionError::BooleanEqualTypeMismatch { .. })
    ));
}

#[test]
fn integer_equality_is_typed_and_reduces_closed_terms() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let integer = |value| {
        ScalarTerm::integer(u8_type, IntegerValue::Unsigned(value))
            .expect("u8 literal is representable")
    };
    let equal = ScalarTerm::integer_equal(u8_type, integer(255), integer(255)).unwrap();
    let unequal = ScalarTerm::integer_equal(u8_type, integer(0), integer(255)).unwrap();
    assert_eq!(equal.scalar_type(), ScalarType::Boolean);
    assert_eq!(equal.boolean_value(), Some(true));
    assert_eq!(unequal.boolean_value(), Some(false));

    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
    assert!(matches!(
        ScalarTerm::integer_equal(u8_type, integer(255), signed),
        Err(PropositionError::IntegerEqualTypeMismatch { .. })
    ));
}

#[test]
fn integer_ordering_is_typed_and_respects_signedness() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let unsigned =
        |value| ScalarTerm::integer(u8_type, IntegerValue::Unsigned(value)).expect("u8 literal");
    let less = ScalarTerm::integer_less_than(u8_type, unsigned(1), unsigned(255)).unwrap();
    let less_or_equal =
        ScalarTerm::integer_less_or_equal(u8_type, unsigned(255), unsigned(255)).unwrap();
    assert_eq!(less.scalar_type(), ScalarType::Boolean);
    assert_eq!(less.boolean_value(), Some(true));
    assert_eq!(less_or_equal.boolean_value(), Some(true));

    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed =
        |value| ScalarTerm::integer(i8_type, IntegerValue::Signed(value)).expect("i8 literal");
    assert_eq!(
        ScalarTerm::integer_less_than(i8_type, signed(-1), signed(0))
            .unwrap()
            .boolean_value(),
        Some(true)
    );
    assert!(matches!(
        ScalarTerm::integer_less_than(u8_type, unsigned(1), signed(-1)),
        Err(PropositionError::IntegerOperandTypeMismatch { .. })
    ));
}

#[test]
fn integer_bitwise_operations_are_typed_and_reduce_closed_terms() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let unsigned =
        |value| ScalarTerm::integer(u8_type, IntegerValue::Unsigned(value)).expect("u8 literal");
    let and = ScalarTerm::integer_bitwise_and(u8_type, unsigned(0b1100), unsigned(0b1010))
        .expect("matching integer operands");
    let or = ScalarTerm::integer_bitwise_or(u8_type, unsigned(0b1100), unsigned(0b0011))
        .expect("matching integer operands");
    let xor = ScalarTerm::integer_bitwise_xor(u8_type, unsigned(0b1100), unsigned(0b1010))
        .expect("matching integer operands");
    let not = ScalarTerm::integer_bitwise_not(u8_type, unsigned(0b0000_1111))
        .expect("matching integer operand");
    assert_eq!(and.scalar_type(), ScalarType::Integer(u8_type));
    assert_eq!(
        and.integer_value(),
        Some((u8_type, IntegerValue::Unsigned(0b1000)))
    );
    assert_eq!(
        or.integer_value(),
        Some((u8_type, IntegerValue::Unsigned(0b1111)))
    );
    assert_eq!(
        xor.integer_value(),
        Some((u8_type, IntegerValue::Unsigned(0b0110)))
    );
    assert_eq!(
        not.integer_value(),
        Some((u8_type, IntegerValue::Unsigned(0b1111_0000)))
    );

    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed =
        |value| ScalarTerm::integer(i8_type, IntegerValue::Signed(value)).expect("i8 literal");
    assert_eq!(
        ScalarTerm::integer_bitwise_xor(i8_type, signed(-1), signed(-128))
            .unwrap()
            .integer_value(),
        Some((i8_type, IntegerValue::Signed(127)))
    );
    assert_eq!(
        ScalarTerm::integer_bitwise_not(i8_type, signed(-128))
            .unwrap()
            .integer_value(),
        Some((i8_type, IntegerValue::Signed(127)))
    );
    assert!(matches!(
        ScalarTerm::integer_bitwise_not(u8_type, signed(1)),
        Err(PropositionError::IntegerBitwiseNotTypeMismatch { .. })
    ));
    assert!(matches!(
        ScalarTerm::integer_bitwise_and(u8_type, unsigned(1), signed(1)),
        Err(PropositionError::IntegerOperandTypeMismatch { .. })
    ));
}

#[test]
fn wrapping_shifts_reduce_counts_modulo_the_value_width() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    assert_eq!(
        u8_type.wrapping_shift_left(
            IntegerValue::Unsigned(1),
            u16_type,
            IntegerValue::Unsigned(9),
        ),
        Some(IntegerValue::Unsigned(2))
    );
    assert_eq!(
        i8_type.wrapping_shift_right(
            IntegerValue::Signed(-8),
            u16_type,
            IntegerValue::Unsigned(9),
        ),
        Some(IntegerValue::Signed(-4))
    );
    assert_eq!(
        u8_type.wrapping_shift_left(IntegerValue::Unsigned(1), i8_type, IntegerValue::Signed(-1),),
        Some(IntegerValue::Unsigned(128))
    );

    // Terminal Psi admits exact widths that are not native source widths;
    // modulo is the semantic rule, rather than a power-of-two bit mask.
    let u6_type = IntegerType::new(IntegerSign::Unsigned, 6).expect("u6");
    assert_eq!(
        u6_type.wrapping_shift_left(
            IntegerValue::Unsigned(1),
            u16_type,
            IntegerValue::Unsigned(7),
        ),
        Some(IntegerValue::Unsigned(2))
    );

    let term = ScalarTerm::wrapping_integer_shift_left(
        u8_type,
        u16_type,
        ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).unwrap(),
        ScalarTerm::integer(u16_type, IntegerValue::Unsigned(9)).unwrap(),
    )
    .expect("independently typed count");
    assert_eq!(
        term.integer_value(),
        Some((u8_type, IntegerValue::Unsigned(2)))
    );
}

#[test]
fn exact_right_shifts_require_an_in_range_nonnegative_count() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");

    assert_eq!(
        u8_type.exact_shift_right(
            IntegerValue::Unsigned(0b1000_0000),
            i16_type,
            IntegerValue::Signed(7),
        ),
        Some(IntegerValue::Unsigned(1))
    );
    assert_eq!(
        i8_type.exact_shift_right(IntegerValue::Signed(-8), i16_type, IntegerValue::Signed(2),),
        Some(IntegerValue::Signed(-2))
    );
    assert_eq!(
        u8_type.exact_shift_right(
            IntegerValue::Unsigned(1),
            i16_type,
            IntegerValue::Signed(-1),
        ),
        None
    );
    assert_eq!(
        u8_type.exact_shift_right(IntegerValue::Unsigned(1), i16_type, IntegerValue::Signed(8),),
        None
    );

    let term = ScalarTerm::exact_integer_shift_right(
        u8_type,
        i16_type,
        ScalarTerm::integer(u8_type, IntegerValue::Unsigned(128)).unwrap(),
        ScalarTerm::integer(i16_type, IntegerValue::Signed(7)).unwrap(),
    )
    .expect("exact right shift with an independently typed count");
    assert_eq!(
        term.integer_value(),
        Some((u8_type, IntegerValue::Unsigned(1)))
    );
}

#[test]
fn exact_left_shifts_require_a_legal_count_and_representable_result() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");

    assert_eq!(
        u8_type.exact_shift_left(IntegerValue::Unsigned(1), i16_type, IntegerValue::Signed(7),),
        Some(IntegerValue::Unsigned(128))
    );
    assert_eq!(
        u8_type.exact_shift_left(IntegerValue::Unsigned(2), i16_type, IntegerValue::Signed(7),),
        None
    );
    assert_eq!(
        i8_type.exact_shift_left(IntegerValue::Signed(-1), i16_type, IntegerValue::Signed(7),),
        Some(IntegerValue::Signed(-128))
    );
    assert_eq!(
        i8_type.exact_shift_left(IntegerValue::Signed(1), i16_type, IntegerValue::Signed(-1),),
        None
    );

    let term = ScalarTerm::exact_integer_shift_left(
        u8_type,
        i16_type,
        ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).unwrap(),
        ScalarTerm::integer(i16_type, IntegerValue::Signed(7)).unwrap(),
    )
    .expect("exact left shift with an independently typed count");
    assert_eq!(
        term.integer_value(),
        Some((u8_type, IntegerValue::Unsigned(128)))
    );
}

#[test]
fn proposition_context_rejects_value_type_reinterpretation() {
    let id = ValueId::new(7).expect("value identity");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 type");
    let context = PropositionContext::from_value_types([(id, ScalarType::Integer(i32_type))])
        .expect("value context");
    let proposition = Proposition::Equal(
        ScalarTerm::value(id, ScalarType::Boolean),
        ScalarTerm::boolean(true),
    );
    assert!(matches!(
        context.validate(&proposition),
        Err(PropositionError::ValueTypeMismatch { .. })
    ));
}

#[test]
fn wrapping_add_reduces_at_the_declared_width_for_all_edge_shapes() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        u8_type.wrapping_add(IntegerValue::Unsigned(200), IntegerValue::Unsigned(100)),
        Some(IntegerValue::Unsigned(44))
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_eq!(
        i8_type.wrapping_add(IntegerValue::Signed(120), IntegerValue::Signed(20)),
        Some(IntegerValue::Signed(-116))
    );
    let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    assert_eq!(
        u128_type.wrapping_add(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(1)),
        Some(IntegerValue::Unsigned(0))
    );
    let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
    assert_eq!(
        i128_type.wrapping_add(IntegerValue::Signed(i128::MAX), IntegerValue::Signed(1)),
        Some(IntegerValue::Signed(i128::MIN))
    );
}

#[test]
fn exact_add_rejects_sums_outside_the_declared_carrier() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        u8_type.exact_add(IntegerValue::Unsigned(200), IntegerValue::Unsigned(55)),
        Some(IntegerValue::Unsigned(255))
    );
    assert_eq!(
        u8_type.exact_add(IntegerValue::Unsigned(200), IntegerValue::Unsigned(56)),
        None
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_eq!(
        i8_type.exact_add(IntegerValue::Signed(120), IntegerValue::Signed(7)),
        Some(IntegerValue::Signed(127))
    );
    assert_eq!(
        i8_type.exact_add(IntegerValue::Signed(-120), IntegerValue::Signed(-9)),
        None
    );
    let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    assert_eq!(
        u128_type.exact_add(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(1)),
        None
    );
    let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
    assert_eq!(
        i128_type.exact_add(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(-1)),
        None
    );
}

#[test]
fn exact_sub_rejects_differences_outside_the_declared_carrier() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        u8_type.exact_sub(IntegerValue::Unsigned(5), IntegerValue::Unsigned(5)),
        Some(IntegerValue::Unsigned(0))
    );
    assert_eq!(
        u8_type.exact_sub(IntegerValue::Unsigned(4), IntegerValue::Unsigned(5)),
        None
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_eq!(
        i8_type.exact_sub(IntegerValue::Signed(-120), IntegerValue::Signed(8)),
        Some(IntegerValue::Signed(-128))
    );
    assert_eq!(
        i8_type.exact_sub(IntegerValue::Signed(-121), IntegerValue::Signed(8)),
        None
    );
    assert_eq!(
        i8_type.exact_sub(IntegerValue::Signed(120), IntegerValue::Signed(-7)),
        Some(IntegerValue::Signed(127))
    );
    assert_eq!(
        i8_type.exact_sub(IntegerValue::Signed(121), IntegerValue::Signed(-7)),
        None
    );
    let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    assert_eq!(
        u128_type.exact_sub(IntegerValue::Unsigned(0), IntegerValue::Unsigned(1)),
        None
    );
    let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
    assert_eq!(
        i128_type.exact_sub(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(1)),
        None
    );
}

#[test]
fn exact_mul_rejects_products_outside_the_declared_carrier() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        u8_type.exact_mul(IntegerValue::Unsigned(51), IntegerValue::Unsigned(5)),
        Some(IntegerValue::Unsigned(255))
    );
    assert_eq!(
        u8_type.exact_mul(IntegerValue::Unsigned(52), IntegerValue::Unsigned(5)),
        None
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_eq!(
        i8_type.exact_mul(IntegerValue::Signed(-42), IntegerValue::Signed(3)),
        Some(IntegerValue::Signed(-126))
    );
    assert_eq!(
        i8_type.exact_mul(IntegerValue::Signed(-43), IntegerValue::Signed(3)),
        None
    );
    assert_eq!(
        i8_type.exact_mul(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
        None
    );
    let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    assert_eq!(
        u128_type.exact_mul(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(2)),
        None
    );
    let term = ScalarTerm::exact_integer_multiply(
        u8_type,
        ScalarTerm::integer(u8_type, IntegerValue::Unsigned(51)).unwrap(),
        ScalarTerm::integer(u8_type, IntegerValue::Unsigned(5)).unwrap(),
    )
    .expect("exact multiply term");
    assert_eq!(
        term.integer_value(),
        Some((u8_type, IntegerValue::Unsigned(255)))
    );
}

#[test]
fn exact_div_rejects_zero_and_signed_overflow() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        u8_type.exact_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
        Some(IntegerValue::Unsigned(51))
    );
    assert_eq!(
        u8_type.exact_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
        None
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_eq!(
        i8_type.exact_div(IntegerValue::Signed(-127), IntegerValue::Signed(-1)),
        Some(IntegerValue::Signed(127))
    );
    assert_eq!(
        i8_type.exact_div(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
        None
    );
}

#[test]
fn exact_rem_is_truncating_and_rejects_undefined_quotients() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        u8_type.exact_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
        Some(IntegerValue::Unsigned(0))
    );
    assert_eq!(
        u8_type.exact_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
        None
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_eq!(
        i8_type.exact_rem(IntegerValue::Signed(-127), IntegerValue::Signed(5)),
        Some(IntegerValue::Signed(-2))
    );
    assert_eq!(
        i8_type.exact_rem(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
        None
    );
}

#[test]
fn wrapping_div_reduces_the_signed_minimum_quotient_overflow() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        u8_type.wrapping_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
        Some(IntegerValue::Unsigned(51))
    );
    assert_eq!(
        u8_type.wrapping_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
        None
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_eq!(
        i8_type.wrapping_div(IntegerValue::Signed(-127), IntegerValue::Signed(-1)),
        Some(IntegerValue::Signed(127))
    );
    assert_eq!(
        i8_type.wrapping_div(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
        Some(IntegerValue::Signed(-128))
    );
}

#[test]
fn wrapping_rem_reduces_the_signed_minimum_quotient_overflow_to_zero() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        u8_type.wrapping_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
        Some(IntegerValue::Unsigned(0))
    );
    assert_eq!(
        u8_type.wrapping_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
        None
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_eq!(
        i8_type.wrapping_rem(IntegerValue::Signed(-127), IntegerValue::Signed(5)),
        Some(IntegerValue::Signed(-2))
    );
    assert_eq!(
        i8_type.wrapping_rem(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
        Some(IntegerValue::Signed(0))
    );
}

#[test]
fn saturating_div_clamps_the_signed_minimum_quotient_overflow() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        u8_type.saturating_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
        Some(IntegerValue::Unsigned(51))
    );
    assert_eq!(
        u8_type.saturating_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
        None
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_eq!(
        i8_type.saturating_div(IntegerValue::Signed(-127), IntegerValue::Signed(-1)),
        Some(IntegerValue::Signed(127))
    );
    assert_eq!(
        i8_type.saturating_div(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
        Some(IntegerValue::Signed(127))
    );
}

#[test]
fn saturating_rem_reduces_the_signed_minimum_quotient_overflow_to_zero() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        u8_type.saturating_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
        Some(IntegerValue::Unsigned(0))
    );
    assert_eq!(
        u8_type.saturating_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
        None
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_eq!(
        i8_type.saturating_rem(IntegerValue::Signed(-127), IntegerValue::Signed(5)),
        Some(IntegerValue::Signed(-2))
    );
    assert_eq!(
        i8_type.saturating_rem(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
        Some(IntegerValue::Signed(0))
    );
}

#[test]
fn saturating_add_clamps_at_declared_signed_and_unsigned_bounds() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        u8_type.saturating_add(IntegerValue::Unsigned(200), IntegerValue::Unsigned(100)),
        Some(IntegerValue::Unsigned(255))
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_eq!(
        i8_type.saturating_add(IntegerValue::Signed(120), IntegerValue::Signed(20)),
        Some(IntegerValue::Signed(127))
    );
    assert_eq!(
        i8_type.saturating_add(IntegerValue::Signed(-120), IntegerValue::Signed(-20)),
        Some(IntegerValue::Signed(-128))
    );
    let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    assert_eq!(
        u128_type.saturating_add(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(1)),
        Some(IntegerValue::Unsigned(u128::MAX))
    );
    let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
    assert_eq!(
        i128_type.saturating_add(IntegerValue::Signed(i128::MAX), IntegerValue::Signed(1)),
        Some(IntegerValue::Signed(i128::MAX))
    );
    assert_eq!(
        i128_type.saturating_add(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(-1)),
        Some(IntegerValue::Signed(i128::MIN))
    );
}

#[test]
fn wrapping_subtract_reduces_at_the_declared_width_for_all_edge_shapes() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        u8_type.wrapping_sub(IntegerValue::Unsigned(5), IntegerValue::Unsigned(10)),
        Some(IntegerValue::Unsigned(251))
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_eq!(
        i8_type.wrapping_sub(IntegerValue::Signed(-120), IntegerValue::Signed(20)),
        Some(IntegerValue::Signed(116))
    );
    assert_eq!(
        i8_type.wrapping_sub(IntegerValue::Signed(120), IntegerValue::Signed(-20)),
        Some(IntegerValue::Signed(-116))
    );
    let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    assert_eq!(
        u128_type.wrapping_sub(IntegerValue::Unsigned(0), IntegerValue::Unsigned(1)),
        Some(IntegerValue::Unsigned(u128::MAX))
    );
    let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
    assert_eq!(
        i128_type.wrapping_sub(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(1)),
        Some(IntegerValue::Signed(i128::MAX))
    );
}

#[test]
fn saturating_subtract_clamps_at_declared_signed_and_unsigned_bounds() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        u8_type.saturating_sub(IntegerValue::Unsigned(5), IntegerValue::Unsigned(10)),
        Some(IntegerValue::Unsigned(0))
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_eq!(
        i8_type.saturating_sub(IntegerValue::Signed(-120), IntegerValue::Signed(20)),
        Some(IntegerValue::Signed(-128))
    );
    assert_eq!(
        i8_type.saturating_sub(IntegerValue::Signed(120), IntegerValue::Signed(-20)),
        Some(IntegerValue::Signed(127))
    );
    let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    assert_eq!(
        u128_type.saturating_sub(IntegerValue::Unsigned(0), IntegerValue::Unsigned(1)),
        Some(IntegerValue::Unsigned(0))
    );
    let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
    assert_eq!(
        i128_type.saturating_sub(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(1)),
        Some(IntegerValue::Signed(i128::MIN))
    );
    assert_eq!(
        i128_type.saturating_sub(IntegerValue::Signed(i128::MAX), IntegerValue::Signed(-1)),
        Some(IntegerValue::Signed(i128::MAX))
    );
}

#[test]
fn wrapping_multiply_reduces_at_the_declared_width_for_all_edge_shapes() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        u8_type.wrapping_mul(IntegerValue::Unsigned(20), IntegerValue::Unsigned(13)),
        Some(IntegerValue::Unsigned(4))
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_eq!(
        i8_type.wrapping_mul(IntegerValue::Signed(20), IntegerValue::Signed(7)),
        Some(IntegerValue::Signed(-116))
    );
    assert_eq!(
        i8_type.wrapping_mul(IntegerValue::Signed(-20), IntegerValue::Signed(7)),
        Some(IntegerValue::Signed(116))
    );
    let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    assert_eq!(
        u128_type.wrapping_mul(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(2)),
        Some(IntegerValue::Unsigned(u128::MAX - 1))
    );
    let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
    assert_eq!(
        i128_type.wrapping_mul(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(-1)),
        Some(IntegerValue::Signed(i128::MIN))
    );
}

#[test]
fn deeply_nested_terms_validate_off_the_call_stack() {
    // The validators used to recurse on their trees, so a term nested deeper
    // than the thread stack overflowed before validation could answer. The
    // worklists keep the same first-error order without spending stack.
    // `forget` skips the still-recursive drop glue on the boxed trees.
    const DEPTH: usize = 100_000;

    let mut math = IntegerMathTerm::literal(IntegerValue::Unsigned(0));
    for _ in 0..DEPTH {
        math = IntegerMathTerm::Add(
            Box::new(math),
            Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
        );
    }
    assert_eq!(math.validate(), Ok(()));
    std::mem::forget(math);

    let mut scalar = ScalarTerm::boolean(true);
    for _ in 0..DEPTH {
        scalar = ScalarTerm::BooleanNot {
            operand: Box::new(scalar),
        };
    }
    assert_eq!(scalar.validate(), Ok(()));
    std::mem::forget(scalar);

    // Errors still surface from the bottom of the nest first: the innermost
    // `BooleanNot` sees an integer operand and rejects before its parents run.
    let mut bad_scalar = ScalarTerm::integer(
        IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
        IntegerValue::Unsigned(1),
    )
    .expect("u8 literal");
    for _ in 0..DEPTH {
        bad_scalar = ScalarTerm::BooleanNot {
            operand: Box::new(bad_scalar),
        };
    }
    assert_eq!(
        bad_scalar.validate(),
        Err(PropositionError::BooleanNotTypeMismatch(
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"))
        ))
    );
    std::mem::forget(bad_scalar);

    let mut proposition = Proposition::Truth;
    for _ in 0..DEPTH {
        proposition = Proposition::Implication {
            premise: Box::new(Proposition::Truth),
            conclusion: Box::new(proposition),
        };
    }
    assert_eq!(proposition.validate(), Ok(()));
    let context = PropositionContext::from_value_types([]).expect("empty context");
    assert_eq!(context.validate(&proposition), Ok(()));
    std::mem::forget(proposition);

    let mut proof_term = ProofTerm::Formal { position: 0 };
    for _ in 0..DEPTH {
        proof_term = ProofTerm::Construction {
            type_identity: "Nat".to_owned(),
            case_identity: None,
            fields: vec![ProofTermField {
                field_identity: "pred".to_owned(),
                term: proof_term,
            }],
        };
    }
    assert_eq!(proof_term.validate(), Ok(()));
    std::mem::forget(proof_term);
}

#[test]
fn saturating_multiply_clamps_at_declared_signed_and_unsigned_bounds() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        u8_type.saturating_mul(IntegerValue::Unsigned(20), IntegerValue::Unsigned(13)),
        Some(IntegerValue::Unsigned(255))
    );
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_eq!(
        i8_type.saturating_mul(IntegerValue::Signed(20), IntegerValue::Signed(7)),
        Some(IntegerValue::Signed(127))
    );
    assert_eq!(
        i8_type.saturating_mul(IntegerValue::Signed(-20), IntegerValue::Signed(7)),
        Some(IntegerValue::Signed(-128))
    );
    let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    assert_eq!(
        u128_type.saturating_mul(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(2),),
        Some(IntegerValue::Unsigned(u128::MAX))
    );
    let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
    assert_eq!(
        i128_type.saturating_mul(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(-1),),
        Some(IntegerValue::Signed(i128::MAX))
    );
}
