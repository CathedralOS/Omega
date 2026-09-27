use super::Interval;
use crate::proof_contracts::arithmetic_domains::integer_ranges::u64_exact_shift_left_fits;

fn iv(low: i64, high: i64) -> Interval {
    Interval {
        low: Some(low),
        high: Some(high),
    }
}

#[test]
fn signed_division_preserves_one_sided_nonzero_divisor_bounds() {
    let positive = Interval {
        low: Some(2),
        high: None,
    };
    let negative = Interval {
        low: None,
        high: Some(-2),
    };
    assert_eq!(iv(10, 50).divide(positive), iv(0, 25));
    assert_eq!(iv(-50, -10).divide(positive), iv(-25, 0));
    assert_eq!(iv(10, 50).divide(negative), iv(-25, 0));
    assert_eq!(iv(-50, -10).divide(negative), iv(0, 25));
    assert_eq!(iv(-50, 10).divide(negative), iv(-5, 25));
    assert_eq!(
        iv(i64::MIN, 0).divide(Interval {
            low: None,
            high: Some(-1)
        }),
        Interval::UNBOUNDED
    );
    for divisor in [
        Interval {
            low: Some(0),
            high: None,
        },
        Interval {
            low: Some(-1),
            high: None,
        },
        Interval {
            low: None,
            high: Some(0),
        },
        Interval {
            low: None,
            high: Some(1),
        },
    ] {
        assert_eq!(iv(-50, 10).divide(divisor), Interval::UNBOUNDED);
    }
}

#[test]
fn signed_division_bounds_cover_every_small_interval_value() {
    for low in -4_i64..=4 {
        for high in low..=4 {
            for divisor_low in -4_i64..=4 {
                for divisor_high in divisor_low..=4 {
                    if divisor_low <= 0 && divisor_high >= 0 {
                        continue;
                    }
                    let quotient = iv(low, high).divide(iv(divisor_low, divisor_high));
                    for value in low..=high {
                        for divisor in divisor_low..=divisor_high {
                            let result = value / divisor;
                            assert!(quotient.low.unwrap() <= result);
                            assert!(quotient.high.unwrap() >= result);
                        }
                    }
                }
            }
        }
    }
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
