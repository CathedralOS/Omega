use crate::preparation::generic_data::division_tests::assert_rejected;
use crate::preparation::generic_data::division_tests::instance;
use crate::preparation::generic_data::division_tests::normalize;

#[test]
fn decimal_const_landing_checks_integrality_range_and_operand_type() {
    for (integer_type, expression, expected_error) in [
        ("u64", "7.5", "exact anonymous value `15/2`"),
        ("u64", "7 / 2.0", "exact anonymous value `7/2`"),
        ("u64", "2u64 * (7 / 2.0)", "exact anonymous value `7/2`"),
        ("u64", "1.0 / 0.0", "division by zero"),
        ("u64", "(1 / 0.0) * 0", "division by zero"),
        ("u8", "256.0", "const value `256` does not fit `u8`"),
        ("u8", "-0.1 * 10", "const value `-1` does not fit `u8`"),
        ("i64", "9223372036854775808.0", "does not fit `i64`"),
        ("u64", "18446744073709551616.0", "64-bit envelope"),
        ("u64", "7.0f64", "not a symbolic integer const expression"),
        (
            "u64",
            "7.0f32 / 2 * 2",
            "not a symbolic integer const expression",
        ),
        ("u64", "7.0 % 2", "requires an integer-typed operand"),
    ] {
        assert_rejected(
            &format!(
                "data Integer<const N: {integer_type}> {{ value: i64; }}
                 data Main {{ value: Integer<{expression}>; }}"
            ),
            expected_error,
        );
    }
    for (integer_type, expression, expected) in [
        ("u8", "255.0", 255),
        ("i64", "-9223372036854775808.0", i128::from(i64::MIN)),
        ("u64", "18446744073709551615.0", i128::from(u64::MAX)),
        ("u64", "7u64 / 2.0 * 2", 6),
    ] {
        let source = format!(
            "data Integer<const N: {integer_type}> {{ value: i64; }}
             data Main {{ value: Integer<{expression}>; }}"
        );
        let (syntax, warnings) = normalize(&source).expect("representable integer boundary");
        instance(&syntax, "Integer", expected);
        assert!(warnings.is_empty(), "{source}: {warnings:?}");
    }
}
