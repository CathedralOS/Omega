use crate::tests::front_end::checked_program_result;

fn check(source: &str, accepted: bool, rejection: &str) {
    match checked_program_result(source) {
        Ok(_) => assert!(accepted, "unproved lower bound accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(rejection)),
                "expected {rejection}: {diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn writes_retire_old_lower_bounds_before_a_fresh_upper_guard() {
    for mutation in ["index = -1;", "set(&mut index);"] {
        check(
            &format!(
                "machine set(output: &mut i64) {{ output = -1; }}
             machine read(items: &[u8], mut index: i64) -> u8
             requires 0 <= index && index < items.len;
             {{
                 {mutation}
                 transition index < items.len {{ true -> (items[index]) false -> 0 }}
             }}"
            ),
            false,
            "non-negative",
        );
    }
}

#[test]
fn current_extent_minus_a_valid_offset_proves_both_bounds() {
    for access in ["0..items.len - offset", "..items.len - offset"] {
        check(
            &format!(
                "machine window(items: &[u8], offset: u64)
            requires offset <= items.len;
            {{ let view: &[u8] = items[{access}]; }}"
            ),
            true,
            "",
        );
    }
    check(
        "machine window(items: &[u8]) requires items.len > 0;
        { let view: &[u8] = items[0..items.len - 1]; }",
        true,
        "",
    );
}

#[test]
fn length_subtraction_never_borrows_another_extent_or_an_invalid_offset() {
    for (parameters, requirement, endpoint) in [
        ("offset: u64", "", "items.len - offset"),
        (
            "offset: i64",
            "requires offset <= items.len;",
            "items.len - offset",
        ),
        ("other: &[u8]", "requires other.len > 0;", "other.len - 1"),
        (
            "offset: u64",
            "requires offset <= items.len;",
            "items.len + offset",
        ),
    ] {
        check(
            &format!(
                "machine window(items: &[u8], {parameters}) {requirement}
            {{ let view: &[u8] = items[0..{endpoint}]; }}"
            ),
            false,
            "cannot prove",
        );
    }
}

#[test]
fn zero_offset_does_not_make_the_extent_an_inclusive_endpoint() {
    for (separator, accepted) in [("..", true), ("..=", false)] {
        check(
            &format!(
                "machine window(items: &[u8]) requires items.len > 0;
            {{ let view: &[u8] = items[0{separator}items.len - 0]; }}"
            ),
            accepted,
            "cannot prove",
        );
    }
}
