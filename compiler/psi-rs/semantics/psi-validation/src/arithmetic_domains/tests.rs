use super::{
    Interval, float_interval_fits_integer, integer_interval_fits_primitive,
    u64_exact_shift_left_fits,
};
use psi_typed_trees::types::PrimitiveType;

fn iv(low: i64, high: i64) -> Interval {
    Interval {
        low: Some(low),
        high: Some(high),
    }
}

#[test]
fn modulo_by_positive_constant_bounds_by_divisor() {
    // unknown-sign dividend % 100 -> [-99, 99]
    assert_eq!(Interval::UNBOUNDED.modulo(iv(100, 100)), iv(-99, 99));
    // non-negative dividend -> non-negative remainder
    assert_eq!(iv(0, i64::MAX).modulo(iv(100, 100)), iv(0, 99));
    // non-positive dividend -> non-positive remainder
    assert_eq!(iv(i64::MIN, 0).modulo(iv(100, 100)), iv(-99, 0));
    // divisor given as a positive RANGE uses its max magnitude
    assert_eq!(Interval::UNBOUNDED.modulo(iv(2, 8)), iv(-7, 7));
}

#[test]
fn modulo_unsound_divisors_stay_unbounded() {
    // divisor may be 0 -> cannot bound
    assert_eq!(iv(0, 10).modulo(iv(0, 5)), Interval::UNBOUNDED);
    // divisor spans 0
    assert_eq!(iv(0, 10).modulo(iv(-3, 3)), Interval::UNBOUNDED);
    // unbounded divisor
    assert_eq!(iv(0, 10).modulo(Interval::UNBOUNDED), Interval::UNBOUNDED);
    // negative divisor range bounds by magnitude
    assert_eq!(Interval::UNBOUNDED.modulo(iv(-8, -2)), iv(-7, 7));
}

#[test]
fn divide_by_nonzero_never_grows_magnitude() {
    // A single-valued POSITIVE divisor gives EXACT truncated-quotient
    // bounds (monotone in the dividend for k > 0) -- the tight interval
    // that lets `let tens: u32 [0..=9] = x / 10` store-prove.
    assert_eq!(iv(-99, 99).divide(iv(2, 2)), iv(-49, 49));
    assert_eq!(iv(10, 50).divide(iv(3, 3)), iv(3, 16));
    assert_eq!(iv(-50, -10).divide(iv(3, 3)), iv(-16, -3));
    assert_eq!(iv(0, 99).divide(iv(10, 10)), iv(0, 9));
    // A RANGED nonzero divisor keeps the magnitude-preserving
    // over-approximation, widened to include 0.
    assert_eq!(iv(10, 50).divide(iv(2, 3)), iv(0, 50));
    assert_eq!(iv(-50, -10).divide(iv(2, 3)), iv(-50, 0));
    // unbounded dividend cannot be bounded by division
    assert_eq!(Interval::UNBOUNDED.divide(iv(2, 2)), Interval::UNBOUNDED);
    // maybe-zero divisor: cannot assume magnitude >= 1
    assert_eq!(iv(10, 50).divide(iv(0, 5)), Interval::UNBOUNDED);
}

#[test]
fn exact_left_shift_tracks_value_and_count_extrema() {
    assert_eq!(iv(3, 3).shift_left(iv(5, 5)), iv(96, 96));
    assert_eq!(iv(0, 1).shift_left(iv(0, 31)), iv(0, 1_i64 << 31));
    assert_eq!(iv(-1, 0).shift_left(iv(0, 63)), iv(i64::MIN, 0));
    assert_eq!(iv(-2, 3).shift_left(iv(1, 4)), iv(-32, 48));
}

#[test]
fn exact_left_shift_fails_closed_when_bounds_are_unusable() {
    assert_eq!(
        Interval::UNBOUNDED.shift_left(iv(0, 3)),
        Interval::UNBOUNDED
    );
    assert_eq!(iv(1, 1).shift_left(iv(-1, 3)), Interval::UNBOUNDED);
    assert_eq!(iv(1, 1).shift_left(iv(127, 128)), Interval::UNBOUNDED);
    assert_eq!(iv(0, 2).shift_left(iv(127, 127)), Interval::UNBOUNDED);
    assert_eq!(iv(2, 2).shift_left(iv(62, 62)), Interval::UNBOUNDED);
}

#[test]
fn exact_right_shift_tracks_signed_and_unsigned_source_extrema() {
    assert_eq!(iv(0, 2047).shift_right(iv(3, 3)), iv(0, 255));
    assert_eq!(iv(-1024, 1023).shift_right(iv(3, 3)), iv(-128, 127));
    assert_eq!(iv(-128, 127).shift_right(iv(0, 7)), iv(-128, 127));
    assert_eq!(iv(0, 65535).shift_right(iv(8, 15)), iv(0, 255));
}

#[test]
fn exact_right_shift_fails_closed_when_bounds_are_unusable() {
    assert_eq!(
        Interval::UNBOUNDED.shift_right(iv(0, 3)),
        Interval::UNBOUNDED
    );
    assert_eq!(iv(0, 255).shift_right(iv(-1, 3)), Interval::UNBOUNDED);
    assert_eq!(iv(0, 255).shift_right(iv(0, 64)), Interval::UNBOUNDED);
    assert_eq!(
        iv(0, 255).shift_right(Interval::UNBOUNDED),
        Interval::UNBOUNDED
    );
}

#[test]
fn u64_exact_left_shift_checks_beyond_the_i64_interval_ceiling() {
    assert!(u64_exact_shift_left_fits(
        iv(0, 2_305_843_009_213_693_951),
        iv(0, 3),
    ));
    assert!(u64_exact_shift_left_fits(iv(0, 1), iv(0, 63)));
    assert!(!u64_exact_shift_left_fits(
        iv(0, 2_305_843_009_213_693_952),
        iv(0, 3),
    ));
    assert!(!u64_exact_shift_left_fits(iv(0, 1), iv(0, 64)));
}

#[test]
fn min_max_clamp_against_unbounded() {
    assert_eq!(
        Interval::UNBOUNDED.max_with(iv(0, 0)),
        Interval {
            low: Some(0),
            high: None
        }
    );
    assert_eq!(
        Interval::UNBOUNDED.min_with(iv(100, 100)),
        Interval {
            low: None,
            high: Some(100)
        }
    );
    assert_eq!(iv(0, 50).max_with(iv(10, 10)), iv(10, 50));
    assert_eq!(iv(0, 50).min_with(iv(10, 10)), iv(0, 10));
    // chained clamp: max(seed,0) then min(_,60) -> [0,60]
    assert_eq!(
        Interval::UNBOUNDED.max_with(iv(0, 0)).min_with(iv(60, 60)),
        iv(0, 60)
    );
}

#[test]
fn nonzero_magnitude_bound_requires_excluding_zero() {
    assert_eq!(iv(1, 100).nonzero_magnitude_bound(), Some(100));
    assert_eq!(iv(-100, -1).nonzero_magnitude_bound(), Some(100));
    assert_eq!(iv(0, 100).nonzero_magnitude_bound(), None); // includes 0
    assert_eq!(iv(-5, 5).nonzero_magnitude_bound(), None); // spans 0
    assert_eq!(Interval::UNBOUNDED.nonzero_magnitude_bound(), None);
}

#[test]
fn exact_float_to_wide_integer_rejects_rounded_upper_endpoint() {
    assert!(float_interval_fits_integer(
        i64::MIN as f64,
        9223372036854774784.0,
        PrimitiveType::I64,
    ));
    assert!(!float_interval_fits_integer(
        0.0,
        9223372036854775808.0,
        PrimitiveType::I64,
    ));
    assert!(!float_interval_fits_integer(
        0.0,
        18446744073709551616.0,
        PrimitiveType::U64,
    ));
}

#[test]
fn exact_integer_cast_requires_interval_containment() {
    assert!(integer_interval_fits_primitive(
        iv(i8::MIN as i64, i8::MAX as i64),
        PrimitiveType::I8,
        PrimitiveType::I32,
    ));
    assert!(integer_interval_fits_primitive(
        iv(0, u8::MAX as i64),
        PrimitiveType::U8,
        PrimitiveType::U8,
    ));
    assert!(!integer_interval_fits_primitive(
        iv(-1, i8::MAX as i64),
        PrimitiveType::I8,
        PrimitiveType::U8,
    ));
    assert!(!integer_interval_fits_primitive(
        iv(0, 300),
        PrimitiveType::I32,
        PrimitiveType::U8,
    ));
    assert!(integer_interval_fits_primitive(
        Interval {
            low: Some(0),
            high: None,
        },
        PrimitiveType::U64,
        PrimitiveType::U64,
    ));
    assert!(!integer_interval_fits_primitive(
        Interval {
            low: Some(0),
            high: None,
        },
        PrimitiveType::U64,
        PrimitiveType::I64,
    ));
    // Abstract arithmetic can exceed its runtime carrier. The carrier
    // remains an intrinsic bound, so same-carrier policy erasure is exact.
    assert!(integer_interval_fits_primitive(
        Interval::UNBOUNDED,
        PrimitiveType::U32,
        PrimitiveType::U32,
    ));
    // A wrapped i8 computation may have a pre-wrap mathematical interval
    // outside i8. It cannot use an empty intersection to prove that the
    // actual (possibly negative) i8 result fits u8.
    assert!(!integer_interval_fits_primitive(
        iv(200, 200),
        PrimitiveType::I8,
        PrimitiveType::U8,
    ));
}
