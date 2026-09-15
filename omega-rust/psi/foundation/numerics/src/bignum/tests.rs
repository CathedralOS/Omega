//! Big number tests.

use super::{BigInt, BigRational, ExactFloat};

fn big(value: i128) -> BigInt {
    BigInt::from_i128(value)
}

#[test]
fn zero_is_default_and_signless() {
    assert_eq!(BigInt::default(), BigInt::zero());
    assert_eq!(big(0), big(-0));
    assert!(!big(0).is_negative());
    assert_eq!(big(5).sub(&big(5)), BigInt::zero());
    assert!(!big(5).sub(&big(5)).is_negative());
}

#[test]
fn ring_ops_match_i128_on_a_grid() {
    let samples: &[i128] = &[
        0,
        1,
        -1,
        2,
        -3,
        63,
        64,
        65,
        i128::from(i64::MAX),
        i128::from(i64::MIN),
        i128::from(u64::MAX),
        i128::from(u64::MAX) + 1,
        -(i128::from(u64::MAX) + 7),
        1 << 100,
    ];
    for &a in samples {
        for &b in samples {
            assert_eq!(big(a).add(&big(b)), big(a + b), "{a} + {b}");
            assert_eq!(big(a).sub(&big(b)), big(a - b), "{a} - {b}");
            // Products up to 2^100 * 2^100 overflow i128; guard the oracle.
            if let Some(product) = a.checked_mul(b) {
                assert_eq!(big(a).mul(&big(b)), big(product), "{a} * {b}");
            }
            assert_eq!(big(a).cmp(&big(b)), a.cmp(&b), "cmp {a} {b}");
            if b != 0 {
                let (quotient, remainder) = big(a).div_rem(&big(b)).unwrap();
                assert_eq!(quotient, big(a / b), "{a} / {b}");
                assert_eq!(remainder, big(a % b), "{a} % {b}");
            }
        }
    }
}

#[test]
fn division_by_zero_is_none() {
    assert!(big(42).div_rem(&big(0)).is_none());
}

#[test]
fn multi_limb_mul_div_round_trips() {
    // (2^100 + 7) * (2^90 + 13), then divide back out.
    let a = big((1 << 100) + 7);
    let b = big((1 << 90) + 13);
    let product = a.mul(&b);
    let (quotient, remainder) = product.div_rem(&a).unwrap();
    assert_eq!(quotient, b);
    assert!(remainder.is_zero());
    let (quotient, remainder) = product.div_rem(&b).unwrap();
    assert_eq!(quotient, a);
    assert!(remainder.is_zero());
}

#[test]
fn gcd_is_euclid() {
    assert_eq!(big(12).gcd(&big(18)), big(6));
    assert_eq!(big(-12).gcd(&big(18)), big(6));
    assert_eq!(big(0).gcd(&big(0)), big(0));
    assert_eq!(big(0).gcd(&big(5)), big(5));
    let a = big(1 << 100);
    assert_eq!(a.gcd(&big(1 << 60)), big(1 << 60));
}

#[test]
fn display_and_parse_round_trip() {
    let cases = [
        "0",
        "1",
        "-1",
        "9223372036854775807",
        "9223372036854775808",
        "18446744073709551615",
        "18446744073709551616",
        "-340282366920938463463374607431768211456",
        "10000000000000000000000000000000000000000000000000000000001",
    ];
    for case in cases {
        let value = BigInt::from_decimal_str(case).unwrap();
        assert_eq!(value.to_string(), case, "round trip {case}");
    }
    assert!(BigInt::from_decimal_str("").is_none());
    assert!(BigInt::from_decimal_str("-").is_none());
    assert!(BigInt::from_decimal_str("12x3").is_none());
    assert_eq!(
        BigInt::from_decimal_str("18446744073709551615").unwrap(),
        BigInt::from_u64(u64::MAX)
    );
    assert_eq!(
        BigInt::from_str_radix("ffffffffffffffff", 16).unwrap(),
        BigInt::from_u64(u64::MAX)
    );
    assert_eq!(
        BigInt::from_str_radix("-ff", 16).unwrap(),
        BigInt::from_i64(-255)
    );
    assert_eq!(
        BigInt::from_str_radix("10000000000000000", 16).unwrap(),
        BigInt::from_u128(1u128 << 64)
    );
    assert_eq!(
        BigInt::from_str_radix("777", 8).unwrap(),
        BigInt::from_i64(0o777)
    );
    assert_eq!(
        BigInt::from_str_radix("1010", 2).unwrap(),
        BigInt::from_i64(10)
    );
    assert!(BigInt::from_str_radix("f", 8).is_none());
}

#[test]
fn narrowing_conversions_are_exact() {
    assert_eq!(big(i128::from(i64::MAX)).to_i64(), Some(i64::MAX));
    assert_eq!(big(i128::from(i64::MIN)).to_i64(), Some(i64::MIN));
    assert_eq!(big(i128::from(i64::MAX) + 1).to_i64(), None);
    assert_eq!(big(i128::from(i64::MIN) - 1).to_i64(), None);
    assert_eq!(big(i128::from(u64::MAX)).to_u64(), Some(u64::MAX));
    assert_eq!(big(i128::from(u64::MAX) + 1).to_u64(), None);
    assert_eq!(big(-1).to_u64(), None);
    assert_eq!(big(0).to_u64(), Some(0));
}

#[test]
fn rational_exact_integer_conversion_requires_complete_cancellation() {
    let seven = BigRational::from_integer(BigInt::from_u64(7));
    let two = BigRational::from_integer(BigInt::from_u64(2));
    let quotient = seven.div(&two).unwrap();
    assert_eq!(quotient.to_integer_exact(), None);
    assert_eq!(quotient.to_string(), "7/2");
    assert_eq!(
        quotient.mul(&two).to_integer_exact(),
        Some(BigInt::from_u64(7))
    );
    assert_eq!(
        quotient.add(&quotient).to_integer_exact(),
        Some(BigInt::from_u64(7))
    );
    assert_eq!(
        quotient.sub(&quotient).to_integer_exact(),
        Some(BigInt::zero())
    );
    assert_eq!(quotient.negate().to_integer_exact(), None);
    assert_eq!(
        quotient.negate().mul(&two).to_integer_exact(),
        Some(BigInt::from_i64(-7))
    );
    assert!(seven.div(&BigRational::zero()).is_none());
}

#[test]
fn rational_display_normalizes_fraction_sign_and_zero() {
    for (numerator, denominator, expected) in [
        (6, 8, "3/4"),
        (-6, 8, "-3/4"),
        (6, -8, "-3/4"),
        (-6, -8, "3/4"),
        (-8, 2, "-4"),
        (8, -2, "-4"),
        (0, -2, "0"),
    ] {
        let value = BigRational::from_integer(BigInt::from_i64(numerator))
            .div(&BigRational::from_integer(BigInt::from_i64(denominator)))
            .unwrap();
        assert_eq!(value.to_string(), expected);
    }
    let negative_zero = BigRational::from_decimal_str("-0.000").unwrap();
    assert_eq!(negative_zero.to_string(), "0");
    assert_eq!(negative_zero.to_integer_exact(), Some(BigInt::zero()));
    assert_eq!(
        BigRational::from_decimal_str("1.2500").unwrap().to_string(),
        "5/4"
    );
}

#[test]
fn rational_exact_conversion_and_display_preserve_unbounded_values() {
    let huge = BigInt::from_u64(1).shl_bits(256).add(&BigInt::from_u64(1));
    let whole = BigRational::from_integer(huge.clone());
    assert_eq!(whole.to_integer_exact(), Some(huge.clone()));
    assert_eq!(whole.to_string(), huge.to_string());
    let fraction = BigRational::from_integer(huge.mul(&BigInt::from_u64(7)))
        .div(&BigRational::from_integer(BigInt::from_u64(21)))
        .unwrap();
    assert_eq!(fraction.to_integer_exact(), None);
    assert_eq!(fraction.to_string(), format!("{huge}/3"));
    let cancelled = fraction.mul(&BigRational::from_integer(BigInt::from_u64(3)));
    assert_eq!(cancelled.to_integer_exact(), Some(huge.clone()));
    assert_eq!(cancelled.negate().to_integer_exact(), Some(huge.negate()));
    assert_eq!(cancelled.negate().to_string(), format!("-{huge}"));
}

#[test]
fn rational_decimal_arithmetic_stays_exact_until_rounding() {
    let one_tenth = BigRational::from_decimal_str("0.1").unwrap();
    let two_tenths = BigRational::from_decimal_str("0.2").unwrap();
    let exact = one_tenth.add(&two_tenths);
    assert_eq!(exact.to_f64().to_bits(), 0.3f64.to_bits());
    assert_ne!((0.1f64 + 0.2f64).to_bits(), 0.3f64.to_bits());

    let quotient = BigRational::from_decimal_str("1")
        .unwrap()
        .div(&BigRational::from_decimal_str("3").unwrap())
        .unwrap();
    assert_eq!(quotient.to_f32().to_bits(), (1.0f32 / 3.0).to_bits());
}

#[test]
fn rational_rounds_directly_to_binary32_with_ties_to_even() {
    let witness = BigRational::from_decimal_str("8388609.499999999999999").unwrap();
    assert_eq!(witness.to_f32().to_bits(), 0x4b00_0001);

    let halfway = BigRational::from_decimal_str("1.000000059604644775390625").unwrap();
    assert_eq!(halfway.to_f32().to_bits(), 1.0f32.to_bits());
    let above =
        BigRational::from_decimal_str("1.0000000596046447753906250000000000000000000000001")
            .unwrap();
    assert_eq!(above.to_f32().to_bits(), 1.0f32.to_bits() + 1);
}

#[test]
fn rational_ieee_conversion_handles_zero_subnormal_and_overflow() {
    assert_eq!(
        BigRational::from_decimal_str("-0.0")
            .unwrap()
            .to_f32()
            .to_bits(),
        1 << 31
    );
    assert_eq!(
        BigRational::from_decimal_str("1e-50").unwrap().to_f32(),
        0.0
    );
    assert_eq!(
        BigRational::from_decimal_str(
            "1.401298464324817070923729583289916131280261941876515771757068283e-45",
        )
        .unwrap()
        .to_f32()
        .to_bits(),
        1
    );
    assert!(
        BigRational::from_decimal_str("1e100")
            .unwrap()
            .to_f32()
            .is_infinite()
    );
    assert!(
        BigRational::from_decimal_str("1e400")
            .unwrap()
            .to_f64()
            .is_infinite()
    );
}

#[test]
fn exact_float_arithmetic_produces_specials_at_landing() {
    let zero = ExactFloat::from_decimal_str("0.0").unwrap();
    let negative_zero = ExactFloat::from_decimal_str("-0.0").unwrap();
    let one = ExactFloat::from_decimal_str("1.0").unwrap();
    assert!(zero.div(&zero).to_f64().is_nan());
    assert_eq!(one.div(&zero).to_f32(), f32::INFINITY);
    assert_eq!(one.div(&negative_zero).to_f32(), f32::NEG_INFINITY);
    assert!(
        one.div(&zero)
            .add(&one.div(&negative_zero))
            .to_f64()
            .is_nan()
    );
    assert_eq!(
        zero.div(&one.div(&zero)).to_f32().to_bits(),
        0.0f32.to_bits()
    );
}

#[test]
fn rational_decimal_rounding_matches_rust_parsers_on_a_grid() {
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for _ in 0..2_000 {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let whole = seed % 10_000_000_000;
        seed = seed.rotate_left(17) ^ 0xa076_1d64_78bd_642f;
        let fraction = seed % 1_000_000_000;
        let exponent = (seed.rotate_left(11) % 241) as i32 - 120;
        let sign = if seed & 1 == 0 { "" } else { "-" };
        let text = format!("{sign}{whole}.{fraction:09}e{exponent}");
        let exact = BigRational::from_decimal_str(&text).unwrap();
        assert_eq!(
            exact.clone().to_f32().to_bits(),
            text.parse::<f32>().unwrap().to_bits(),
            "binary32 rounding for {text}"
        );
        assert_eq!(
            exact.to_f64().to_bits(),
            text.parse::<f64>().unwrap().to_bits(),
            "binary64 rounding for {text}"
        );
    }
}
