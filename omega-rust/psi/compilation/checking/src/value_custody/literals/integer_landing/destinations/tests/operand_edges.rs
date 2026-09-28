use super::super::anonymous_integer_landing_warnings;
use super::{typed, width_grants};

#[test]
fn invalid_or_already_typed_arithmetic_receives_no_anonymous_width_grant() {
    for value in [
        "18446744073709551616 / 3",
        "18446744073709551616",
        "18446744073709551616 / 2u64",
        "18446744073709551616 % 2",
    ] {
        for expression in [format!("({value}) | 8u64"), format!("({value}) as u64")] {
            let source = format!("machine result() -> u64 {{ {expression} }}");
            assert!(width_grants(&typed(&source)).is_empty(), "{source}");
        }
    }
    for value in ["7 / 2", "18446744073709551616", "7u64 / 2 * 2"] {
        for expression in [format!("({value}) | 8u64"), format!("({value}) as u64")] {
            let source = format!("machine result() -> u64 {{ {expression} }}");
            assert!(
                anonymous_integer_landing_warnings(&typed(&source)).is_empty(),
                "{source}"
            );
        }
    }
}
