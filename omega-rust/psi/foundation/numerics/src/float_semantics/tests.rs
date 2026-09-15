//! Float semantics tests.

use crate::bignum::{BigInt, ExactFloat};

use super::{
    FloatClass, FloatFormat, FloatMeaning, FloatPolicyTrap, FloatSemantics, FloatToIntegerError,
    IntegerFormat, RoundingDirection,
};

fn meaning32(value: f32) -> FloatMeaning {
    FloatMeaning::from_f32(value)
}

#[test]
fn format_records_match_the_core_surface() {
    assert_eq!(FloatFormat::BINARY32.precision, 24);
    assert_eq!(FloatFormat::BINARY32.minimum_subnormal_exponent, -149);
    assert_eq!(FloatFormat::BINARY64.precision, 53);
    assert_eq!(FloatFormat::BINARY64.minimum_subnormal_exponent, -1074);
}

#[test]
fn decode_encode_round_trips_non_nan_binary_values() {
    for bits in [
        0u32,
        1,
        0x007f_ffff,
        0x0080_0000,
        0x3f80_0000,
        0x7f7f_ffff,
        0x7f80_0000,
        0x8000_0000,
        0xff80_0000,
    ] {
        let value = f32::from_bits(bits);
        assert_eq!(FloatMeaning::from_f32(value).to_f32().to_bits(), bits);
    }
    for bits in [
        0u64,
        1,
        0x000f_ffff_ffff_ffff,
        0x0010_0000_0000_0000,
        0x3ff0_0000_0000_0000,
        0x7fef_ffff_ffff_ffff,
        0x7ff0_0000_0000_0000,
        0x8000_0000_0000_0000,
        0xfff0_0000_0000_0000,
    ] {
        let value = f64::from_bits(bits);
        assert_eq!(FloatMeaning::from_f64(value).to_f64().to_bits(), bits);
    }
}

#[test]
fn binary32_arithmetic_rounds_at_each_named_operation() {
    let large = meaning32(16_777_216.0);
    let one = meaning32(1.0);
    let rounded_sum = FloatSemantics::add(FloatFormat::BINARY32, &large, &one);
    assert_eq!(rounded_sum.to_f32(), 16_777_216.0);
    let difference = FloatSemantics::subtract(FloatFormat::BINARY32, &rounded_sum, &large);
    assert_eq!(difference.to_f32().to_bits(), 0.0f32.to_bits());
}

#[test]
fn specials_and_partial_comparisons_follow_the_settled_laws() {
    let positive_zero = meaning32(0.0);
    let negative_zero = meaning32(-0.0);
    let one = meaning32(1.0);
    let infinity = FloatSemantics::divide(FloatFormat::BINARY32, &one, &positive_zero);
    let negative_infinity = FloatSemantics::divide(FloatFormat::BINARY32, &one, &negative_zero);
    let nan = FloatSemantics::divide(FloatFormat::BINARY32, &positive_zero, &positive_zero);

    assert_eq!(infinity.to_f32(), f32::INFINITY);
    assert_eq!(negative_infinity.to_f32(), f32::NEG_INFINITY);
    assert!(nan.is_nan());
    assert!(!FloatSemantics::equal(&nan, &nan));
    assert!(FloatSemantics::not_equal(&nan, &nan));
    assert!(!FloatSemantics::less(&nan, &one));
    assert!(!FloatSemantics::greater_or_equal(&nan, &one));
    assert_eq!(
        FloatSemantics::minimum(&nan, &one).to_f32().to_bits(),
        one.to_f32().to_bits()
    );
    assert!(FloatSemantics::minimum(&one, &nan).is_nan());
}

#[test]
fn signed_zero_survives_exact_arithmetic_and_rounding() {
    let negative_zero = meaning32(-0.0);
    let sum = FloatSemantics::add(FloatFormat::BINARY32, &negative_zero, &negative_zero);
    assert_eq!(sum.to_f32().to_bits(), (-0.0f32).to_bits());
    let negated = FloatSemantics::negate(FloatFormat::BINARY32, &negative_zero);
    assert_eq!(negated.to_f32().to_bits(), 0.0f32.to_bits());
}

#[test]
fn fused_and_multiply_then_add_are_distinct_operations() {
    let left = meaning32(f32::from_bits(0x3f80_0001));
    let right = left.clone();
    let addend = meaning32(f32::from_bits(0xbf80_0002));
    let unfused = FloatSemantics::multiply_then_add(FloatFormat::BINARY32, &left, &right, &addend);
    let fused = FloatSemantics::fused_multiply_add(FloatFormat::BINARY32, &left, &right, &addend);

    assert_eq!(unfused.to_f32().to_bits(), 0.0f32.to_bits());
    assert_eq!(
        fused.to_f32().to_bits(),
        left.to_f32()
            .mul_add(right.to_f32(), addend.to_f32())
            .to_bits()
    );
    assert_ne!(fused.to_f32().to_bits(), unfused.to_f32().to_bits());
}

#[test]
fn directed_rounding_is_explicit_and_sign_aware() {
    let positive_halfway = ExactFloat::from_decimal_str("1.000000059604644775390625").unwrap();
    let negative_halfway = ExactFloat::from_decimal_str("-1.000000059604644775390625").unwrap();

    let nearest = FloatSemantics::round_exact(FloatFormat::BINARY32, positive_halfway.clone());
    let upward = FloatSemantics::round_exact_directed(
        FloatFormat::BINARY32,
        positive_halfway,
        RoundingDirection::TowardPositive,
    );
    let downward = FloatSemantics::round_exact_directed(
        FloatFormat::BINARY32,
        negative_halfway,
        RoundingDirection::TowardNegative,
    );

    assert_eq!(nearest.to_f32().to_bits(), 1.0f32.to_bits());
    assert_eq!(upward.to_f32().to_bits(), 1.0f32.to_bits() + 1);
    assert_eq!(downward.to_f32().to_bits(), (-1.0f32).to_bits() + 1);
}

#[test]
fn directed_named_arithmetic_rounds_the_exact_operation_result() {
    let one = meaning32(1.0);
    let half_ulp = meaning32(f32::from_bits((127 - 24) << 23));
    let nearest = FloatSemantics::add(FloatFormat::BINARY32, &one, &half_ulp);
    let upward = FloatSemantics::add_toward_positive(FloatFormat::BINARY32, &one, &half_ulp);
    assert_eq!(nearest.to_f32().to_bits(), 1.0f32.to_bits());
    assert_eq!(upward.to_f32().to_bits(), 1.0f32.to_bits() + 1);

    let three = meaning32(3.0);
    let quotient = FloatSemantics::divide_toward_zero(FloatFormat::BINARY32, &one, &three);
    assert_eq!(quotient.to_f32().to_bits(), 0x3eaa_aaaa);
}

#[test]
fn directed_underflow_preserves_the_outward_minimum_subnormal() {
    let minimum_subnormal = ExactFloat::from_f32(f32::from_bits(1));
    let two = ExactFloat::from_decimal_str("2").unwrap();
    let positive_half = minimum_subnormal.div(&two);
    let negative_half = positive_half.negate();

    let nearest = FloatSemantics::round_exact(FloatFormat::BINARY32, positive_half.clone());
    let upward = FloatSemantics::round_exact_directed(
        FloatFormat::BINARY32,
        positive_half,
        RoundingDirection::TowardPositive,
    );
    let downward = FloatSemantics::round_exact_directed(
        FloatFormat::BINARY32,
        negative_half,
        RoundingDirection::TowardNegative,
    );

    assert_eq!(nearest.to_f32().to_bits(), 0.0f32.to_bits());
    assert_eq!(upward.to_f32().to_bits(), 1);
    assert_eq!(downward.to_f32().to_bits(), 0x8000_0001);
}

#[test]
fn directed_overflow_chooses_infinity_or_max_finite_by_sign() {
    let positive = ExactFloat::from_decimal_str("1e1000").unwrap();
    let negative = ExactFloat::from_decimal_str("-1e1000").unwrap();

    let toward_zero = FloatSemantics::round_exact_directed(
        FloatFormat::BINARY32,
        positive.clone(),
        RoundingDirection::TowardZero,
    );
    let upward = FloatSemantics::round_exact_directed(
        FloatFormat::BINARY32,
        positive,
        RoundingDirection::TowardPositive,
    );
    let negative_upward = FloatSemantics::round_exact_directed(
        FloatFormat::BINARY32,
        negative,
        RoundingDirection::TowardPositive,
    );

    assert_eq!(toward_zero.to_f32().to_bits(), f32::MAX.to_bits());
    assert_eq!(upward.to_f32(), f32::INFINITY);
    assert_eq!(negative_upward.to_f32().to_bits(), (-f32::MAX).to_bits());
}

#[test]
fn trapping_policy_checks_the_result_including_propagated_nonfinites() {
    let finite = meaning32(1.0);
    let nan = FloatMeaning::NaN;
    let infinity = FloatMeaning::Infinity { negative: false };

    assert_eq!(
        FloatSemantics::apply_trapping_policy(finite.clone()),
        Ok(finite)
    );
    assert_eq!(
        FloatSemantics::apply_trapping_policy(nan),
        Err(FloatPolicyTrap::NaNResult)
    );
    assert_eq!(
        FloatSemantics::apply_trapping_policy(infinity),
        Err(FloatPolicyTrap::InfinityResult)
    );
}

#[test]
fn saturating_policy_clamps_only_finite_operand_magnitude_overflow() {
    let one = meaning32(1.0);
    let zero = meaning32(0.0);
    let infinity = FloatMeaning::Infinity { negative: false };
    let negative_infinity = FloatMeaning::Infinity { negative: true };
    let nan = FloatMeaning::NaN;

    assert_eq!(
        FloatSemantics::apply_saturating_policy(
            FloatFormat::BINARY32,
            &[&one, &one],
            infinity.clone(),
        )
        .to_f32()
        .to_bits(),
        f32::MAX.to_bits()
    );
    assert_eq!(
        FloatSemantics::apply_saturating_policy(
            FloatFormat::BINARY32,
            &[&one, &one],
            negative_infinity,
        )
        .to_f32()
        .to_bits(),
        (-f32::MAX).to_bits()
    );
    assert_eq!(
        FloatSemantics::apply_saturating_policy(
            FloatFormat::BINARY32,
            &[&infinity, &one],
            infinity.clone(),
        ),
        infinity
    );
    assert_eq!(
        FloatSemantics::apply_saturating_policy(FloatFormat::BINARY32, &[&one, &one], nan.clone(),),
        nan
    );
    assert_eq!(
        FloatSemantics::apply_saturating_policy(
            FloatFormat::BINARY32,
            &[&one, &one, &zero],
            FloatMeaning::Infinity { negative: false },
        )
        .to_f32()
        .to_bits(),
        f32::MAX.to_bits()
    );
    assert_eq!(
        FloatSemantics::apply_saturating_divide_policy(
            FloatFormat::BINARY32,
            &one,
            &zero,
            FloatMeaning::Infinity { negative: false },
        ),
        FloatMeaning::Infinity { negative: false }
    );
}

#[test]
fn classification_distinguishes_normal_subnormal_zero_and_specials() {
    assert_eq!(
        FloatSemantics::classify(FloatFormat::BINARY32, &meaning32(f32::MIN_POSITIVE)),
        FloatClass::Normal { negative: false }
    );
    assert_eq!(
        FloatSemantics::classify(FloatFormat::BINARY32, &meaning32(f32::from_bits(1))),
        FloatClass::Subnormal { negative: false }
    );
    assert_eq!(
        FloatSemantics::classify(FloatFormat::BINARY32, &meaning32(-0.0)),
        FloatClass::Zero { negative: true }
    );
    assert_eq!(
        FloatSemantics::classify(FloatFormat::BINARY32, &meaning32(f32::NEG_INFINITY)),
        FloatClass::Infinity { negative: true }
    );
    assert_eq!(
        FloatSemantics::classify(FloatFormat::BINARY32, &meaning32(f32::NAN)),
        FloatClass::NaN
    );
}

#[test]
fn float_to_integer_operations_share_truncation_and_policy_edges() {
    let negative_fraction = FloatMeaning::from_f64(-0.5);
    let negative_one = FloatMeaning::from_f64(-1.0);
    let positive_overflow = FloatMeaning::from_f64(256.0);
    let nan = FloatMeaning::from_f64(f64::NAN);

    assert_eq!(
        FloatSemantics::to_integer_exact(&negative_fraction, IntegerFormat::U8)
            .unwrap()
            .to_u64(),
        Some(0)
    );
    assert_eq!(
        FloatSemantics::to_integer_exact(&negative_one, IntegerFormat::U8),
        Err(FloatToIntegerError::OutOfRange)
    );
    assert_eq!(
        FloatSemantics::to_integer_trapping(&nan, IntegerFormat::I32),
        Err(FloatToIntegerError::NonFinite)
    );
    assert_eq!(
        FloatSemantics::to_integer_saturating(&positive_overflow, IntegerFormat::U8).to_u64(),
        Some(255)
    );
    assert_eq!(
        FloatSemantics::to_integer_saturating(&nan, IntegerFormat::I32).to_i64(),
        Some(0)
    );
}

#[test]
fn integer_to_float_conversion_rounds_exact_full_width_values() {
    let maximum_u64 = BigInt::from_u64(u64::MAX);
    let nearest = FloatSemantics::from_integer(FloatFormat::BINARY64, &maximum_u64).to_f64();
    let downward =
        FloatSemantics::from_integer_toward_zero(FloatFormat::BINARY64, &maximum_u64).to_f64();

    assert_eq!(nearest.to_bits(), (u64::MAX as f64).to_bits());
    assert!(downward < nearest);
    assert_eq!(
        FloatSemantics::to_integer_saturating(
            &FloatMeaning::from_f64(f64::INFINITY),
            IntegerFormat::U64,
        )
        .to_u64(),
        Some(u64::MAX)
    );
}

#[test]
fn square_root_is_exactly_rounded_without_host_arithmetic() {
    let two32 = meaning32(2.0);
    let nearest32 = FloatSemantics::square_root(FloatFormat::BINARY32, &two32);
    let upward32 = FloatSemantics::square_root_toward_positive(FloatFormat::BINARY32, &two32);
    assert_eq!(nearest32.to_f32().to_bits(), 0x3fb5_04f3);
    assert_eq!(upward32.to_f32().to_bits(), 0x3fb5_04f4);

    let two64 = FloatMeaning::from_f64(2.0);
    let nearest64 = FloatSemantics::square_root(FloatFormat::BINARY64, &two64);
    assert_eq!(nearest64.to_f64().to_bits(), 2.0f64.sqrt().to_bits());

    let perfect = FloatMeaning::from_f64(64.0);
    assert_eq!(
        FloatSemantics::square_root(FloatFormat::BINARY64, &perfect).to_f64(),
        8.0
    );
}

#[test]
fn square_root_preserves_signed_zero_and_rejects_negative_values() {
    let negative_zero = meaning32(-0.0);
    let negative_one = meaning32(-1.0);
    let positive_infinity = meaning32(f32::INFINITY);

    assert_eq!(
        FloatSemantics::square_root(FloatFormat::BINARY32, &negative_zero)
            .to_f32()
            .to_bits(),
        (-0.0f32).to_bits()
    );
    assert!(FloatSemantics::square_root(FloatFormat::BINARY32, &negative_one).is_nan());
    assert_eq!(
        FloatSemantics::square_root(FloatFormat::BINARY32, &positive_infinity).to_f32(),
        f32::INFINITY
    );
}

#[test]
fn square_root_matches_ieee_reference_across_format_boundaries() {
    for bits in [
        1u32,
        2,
        0x007f_ffff,
        0x0080_0000,
        0x3f00_0000,
        0x3f80_0000,
        0x4000_0000,
        0x4b00_0001,
        0x7f7f_ffff,
    ] {
        let value = f32::from_bits(bits);
        let actual =
            FloatSemantics::square_root(FloatFormat::BINARY32, &FloatMeaning::from_f32(value))
                .to_f32();
        assert_eq!(
            actual.to_bits(),
            value.sqrt().to_bits(),
            "binary32 {bits:#x}"
        );
    }
    for bits in [
        1u64,
        0x000f_ffff_ffff_ffff,
        0x0010_0000_0000_0000,
        0x3fe0_0000_0000_0000,
        0x3ff0_0000_0000_0000,
        0x4000_0000_0000_0000,
        0x7fef_ffff_ffff_ffff,
    ] {
        let value = f64::from_bits(bits);
        let actual =
            FloatSemantics::square_root(FloatFormat::BINARY64, &FloatMeaning::from_f64(value))
                .to_f64();
        assert_eq!(
            actual.to_bits(),
            value.sqrt().to_bits(),
            "binary64 {bits:#x}"
        );
    }
}
