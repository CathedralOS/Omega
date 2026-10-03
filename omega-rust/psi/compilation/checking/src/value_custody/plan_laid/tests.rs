use super::stored_integer_bounds;
use layout_plans::IntegerInterpretation::{Signed, Unsigned};

#[test]
fn stored_integer_bounds_preserve_signed_and_full_unsigned_carriers() {
    assert_eq!(stored_integer_bounds(8, Signed), Some((-128, 127)));
    assert_eq!(stored_integer_bounds(8, Unsigned), Some((0, 255)));
    assert_eq!(
        stored_integer_bounds(64, Signed),
        Some((i128::from(i64::MIN), i128::from(i64::MAX)))
    );
    assert_eq!(
        stored_integer_bounds(64, Unsigned),
        Some((0, i128::from(u64::MAX)))
    );
}

#[test]
fn stored_integer_bounds_refuse_invalid_widths_before_shifting() {
    for width in [0, 65, u64::MAX] {
        for interpretation in [Signed, Unsigned] {
            assert_eq!(stored_integer_bounds(width, interpretation), None);
        }
    }
}
