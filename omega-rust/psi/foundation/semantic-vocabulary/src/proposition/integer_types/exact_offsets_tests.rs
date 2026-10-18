//! Closed carrier algebra, independent of source checking and target emission.
use super::{IntegerSign, IntegerType, IntegerValue};

#[test]
fn exact_offset_keeps_independent_address_and_count_widths() {
    let count = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let narrow = IntegerType::address(16).expect("16-bit address");
    let wide = IntegerType::address(128).expect("128-bit address");
    let unsigned = IntegerValue::Unsigned;
    assert_eq!(
        narrow.exact_add_values(narrow, unsigned(65530), count, unsigned(5)),
        Some(unsigned(65535))
    );
    assert_eq!(
        narrow.exact_add_values(count, unsigned(5), narrow, unsigned(65530)),
        Some(unsigned(65535))
    );
    assert_eq!(
        narrow.exact_add_values(narrow, unsigned(65530), count, unsigned(6)),
        None
    );
    assert_eq!(
        narrow.exact_add_values(narrow, unsigned(0), count, unsigned(65536)),
        None
    );
    assert_eq!(
        wide.exact_add_values(wide, unsigned(1_u128 << 80), count, unsigned(3)),
        Some(unsigned((1_u128 << 80) + 3))
    );
    assert_eq!(
        wide.exact_add_values(wide, unsigned(u128::MAX), count, unsigned(1)),
        None
    );
    assert!(!narrow.admits_exact_add_operands(narrow, narrow));
    assert!(!count.admits_exact_add_operands(narrow, count));
    assert!(!narrow.admits_exact_add_operands(
        narrow,
        IntegerType::new(IntegerSign::Unsigned, 32).expect("u32")
    ));
}

#[test]
fn exact_fixed_addition_retains_signed_and_unsigned_bounds() {
    for (sign, left, right, expected) in [
        (
            IntegerSign::Signed,
            IntegerValue::Signed(-128),
            IntegerValue::Signed(1),
            Some(IntegerValue::Signed(-127)),
        ),
        (
            IntegerSign::Signed,
            IntegerValue::Signed(127),
            IntegerValue::Signed(1),
            None,
        ),
        (
            IntegerSign::Unsigned,
            IntegerValue::Unsigned(254),
            IntegerValue::Unsigned(1),
            Some(IntegerValue::Unsigned(255)),
        ),
        (
            IntegerSign::Unsigned,
            IntegerValue::Unsigned(255),
            IntegerValue::Unsigned(1),
            None,
        ),
    ] {
        let carrier = IntegerType::new(sign, 8).expect("8-bit carrier");
        assert_eq!(
            carrier.exact_add_values(carrier, left, carrier, right),
            expected
        );
    }
}
