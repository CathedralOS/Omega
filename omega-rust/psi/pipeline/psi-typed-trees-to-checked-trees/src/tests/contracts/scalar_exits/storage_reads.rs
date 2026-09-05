use super::check;

#[test]
fn scalar_assignments_read_the_previous_storage_value_before_replacing_it() {
    for (value_type, body, expected) in [
        (
            "u8",
            "let mut value: u8 = 3; value = ((value as u8 in Wrapping) + 4) as u8; value",
            "7",
        ),
        ("u8", "let mut value: u8 = 7; value = value; value", "7"),
        (
            "u8",
            "let mut value: u8 = 255; value = ((value as u8 in Wrapping) + 1) as u8; value",
            "0",
        ),
        (
            "u8",
            "let mut value: u8 = 255; value = ((value as u8 in Saturating) + 1) as u8; value",
            "255",
        ),
        (
            "u64",
            "let mut value: u64 = 18446744073709551615u64; value = ((value as u64 in Wrapping) - 1) as u64; value",
            "18446744073709551614u64",
        ),
        (
            "bool",
            "let mut value: bool = false; value = !value; value",
            "true",
        ),
    ] {
        for accepted in [true, false] {
            let comparison = if accepted { "==" } else { "!=" };
            check(
                &format!(
                    "machine produce() -> {value_type} ensures result {comparison} {expected} {{ {body} }}"
                ),
                accepted,
            );
        }
    }
}

#[test]
fn copied_values_remain_distinct_from_later_storage_versions() {
    for (returned, expected) in [("saved", "7"), ("value", "8"), ("last", "7")] {
        check(
            &format!(
                r#"
            machine produce() -> u8 ensures result == {expected} {{
                let first: u8 = 3;
                let mut value: u8 = first;
                let increment: u8 = 4;
                value = ((value as u8 in Wrapping) + (increment as u8 in Wrapping)) as u8;
                let saved: u8 = value;
                value = ((value as u8 in Wrapping) + 1) as u8;
                let last: u8 = saved;
                {returned}
            }}
        "#
            ),
            true,
        );
    }
}

#[test]
fn explicit_state_arguments_keep_completed_mutable_storage_reads() {
    check(
        r#"
        machine produce() -> u8 ensures result == 8 {
            let mut value: u8 = 3;
            value = ((value as u8 in Wrapping) + 4) as u8;
            transition { _ -> finish(value) }
            state finish(input: u8) -> u8 {
                let mut current: u8 = input;
                current = ((current as u8 in Wrapping) + 1) as u8;
                current
            }
        }
    "#,
        true,
    );
}

#[test]
fn unknown_or_invalidated_storage_never_replays_an_initializer() {
    for body in [
        "let mut value: u8 = 3; replace(&mut value); value = ((value as u8 in Wrapping) + 4) as u8; value",
        "let mut value: u8 = 3; value = unknown; value = ((value as u8 in Wrapping) + 4) as u8; value",
        "let mut value: u8 = 3; value = ((value as u8 in Wrapping) + 4) as u8; replace(&mut value); value",
    ] {
        check(
            &format!(
                "machine replace(value: &mut u8) {{ value = 8; }} machine produce(unknown: u8) -> u8 ensures result == 7 {{ {body} }}"
            ),
            false,
        );
    }
}
