use super::*;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(typed_trees(source)) {
        Ok(_) => assert!(accepted, "unexpected cancellation: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{source}\n{diagnostics:#?}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("may overflow")),
                "{source}\n{diagnostics:#?}"
            );
        }
    }
}

#[test]
fn identical_integer_operands_cancel_with_the_exact_zero_bound() {
    for carrier in ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"] {
        check(
            &format!(
                "machine cancel(value: {carrier}, ceiling: {carrier}) -> {carrier} {{ ceiling + (value - value) }}"
            ),
            true,
        );
    }
}

#[test]
fn cancellation_preserves_each_intermediate_overflow_obligation() {
    for expression in [
        "(ceiling + value) - value",
        "ceiling + ((value + 1) - (value + 1))",
        "ceiling + (value - other)",
    ] {
        check(
            &format!(
                "machine cancel(value: u64, other: u64, ceiling: u64) -> u64 {{ {expression} }}"
            ),
            false,
        );
    }
}

#[test]
fn different_pure_call_arguments_cannot_cancel() {
    check(
        "machine identity(value: u64) -> u64 { value } machine cancel(value: u64, other: u64, ceiling: u64) -> u64 { ceiling + (identity(value) - identity(other)) }",
        false,
    );
}

#[test]
fn mutable_calls_do_not_become_equal_by_repeated_spelling() {
    check(
        "machine change(value: &mut u64) -> u64 { let saved: u64 = value; value = 0; saved } machine cancel(input: u64, ceiling: u64) -> u64 { let mut value: u64 = input; ceiling + (change(&mut value) - change(&mut value)) }",
        false,
    );
}

#[test]
fn cancellation_uses_resolved_places_and_builtin_subtraction() {
    check(
        "data Main { value: u64; other: u64; } machine Main::cancel(&self, ceiling: u64) -> u64 { ceiling + (self.value - self.value) }",
        true,
    );
    check(
        "data Main { value: u64; other: u64; } machine Main::cancel(&self, ceiling: u64) -> u64 { ceiling + (self.value - self.other) }",
        false,
    );
    check(
        "operator - u64::custom(left: u64, right: u64) -> u64; machine cancel(value: u64, ceiling: u64) -> u64 { ceiling + (value - value) }",
        false,
    );
    check(
        "machine cancel(input: u64, ceiling: u64) -> u64 { let mut value: u64 = input; value = ceiling; ceiling + (value - value) }",
        true,
    );
}
