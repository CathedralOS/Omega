use super::check;

#[test]
fn computed_local_snapshots_retain_exact_values_and_selected_policies() {
    for (value_type, initializer, expected) in [
        ("u8", "3 + 4", "7"),
        ("u8", "(255 + 1) - 1", "255"),
        ("u64", "18446744073709551614 + 1", "18446744073709551615u64"),
        ("u8", "((255u8 as u8 in Wrapping) + 2) as u8", "1"),
        ("u8", "((255u8 as u8 in Saturating) + 2) as u8", "255"),
        ("bool", "false || true", "true"),
    ] {
        for accepted in [true, false] {
            let comparison = if accepted { "==" } else { "!=" };
            check(
                &format!(
                    "machine produce() -> {value_type} ensures result {comparison} {expected} {{ let value: {value_type} = {initializer}; let saved: {value_type} = value; saved }}"
                ),
                accepted,
            );
        }
    }
}

#[test]
fn computed_local_snapshots_survive_operand_changes_but_not_destination_changes() {
    for (body, expected) in [
        (
            "let input: u8 = 3; let saved: u8 = ((input as u8 in Wrapping) + 4) as u8; replace(&mut input); saved",
            7,
        ),
        (
            "let saved: u8 = 3 + 4; let mut copied: u8 = saved; copied",
            7,
        ),
        (
            "let saved: u8 = 3 + 4; let mut copied: u8 = 0; copied = saved; copied",
            7,
        ),
        (
            "let saved: u8 = 3 + 4; let mut copied: u8 = saved; copied = 8; copied",
            8,
        ),
    ] {
        for accepted in [true, false] {
            let comparison = if accepted { "==" } else { "!=" };
            check(
                &format!(
                    "machine replace(value: &mut u8) {{ value = 8; }} machine produce() -> u8 ensures result {comparison} {expected} {{ {body} }}"
                ),
                accepted,
            );
        }
    }
    check(
        "machine replace(value: &mut u8) { value = 8; } machine produce() -> u8 ensures result == 7 { let saved: u8 = 3 + 4; replace(&mut saved); saved }",
        false,
    );
}

#[test]
fn computed_local_snapshots_follow_renamed_state_arguments() {
    for accepted in [true, false] {
        let comparison = if accepted { "==" } else { "!=" };
        check(
            &format!(
                r#"
            machine produce() -> u8 ensures result {comparison} 8 {{
                let first: u8 = 3 + 4;
                transition {{ _ -> relay(first) }}
                state relay(renamed: u8) -> u8 {{
                    let next: u8 = ((renamed as u8 in Wrapping) + 1) as u8;
                    transition {{ _ -> finish(next) }}
                }}
                state finish(current: u8) -> u8 {{ current }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn computed_state_values_require_all_predecessors_to_agree() {
    for (alternative, accepted) in [("first", true), ("second", false), ("unknown", false)] {
        check(
            &format!(
                r#"
            machine produce(flag: bool, unknown: u8) -> u8 ensures result == 7 {{
                let first: u8 = 3 + 4;
                let second: u8 = 4 + 4;
                transition flag {{ true -> finish(first) false -> finish({alternative}) }}
                state finish(current: u8) -> u8 {{ current }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn computed_snapshots_survive_rebuilt_flow_and_lose_conflicting_late_inputs() {
    for (alternative, accepted) in [("3 + 4", true), ("4 + 4", false), ("unknown", false)] {
        check(
            &format!(
                r#"
            machine produce(flag: bool, unknown: u8) -> u8 ensures result == 7 {{
                let first: u8 = 3 + 4;
                let second: u8 = {alternative};
                transition flag {{ true -> early(first) false -> late(second) }}
                state finish(current: u8) -> u8 {{ current }}
                state early(value: u8) -> u8 {{ transition {{ _ -> finish(value) }} }}
                state late(value: u8) -> u8 {{ transition {{ _ -> finish(value) }} }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn computed_snapshots_copy_into_named_fields_without_following_later_writes() {
    for (body, accepted) in [
        (
            "let saved: u8 = 3 + 4; let packet: Packet = Packet { value: saved }; packet.value",
            true,
        ),
        (
            "let saved: u8 = 3 + 4; let mut packet: Packet = Packet { value: saved }; packet.value = 8; packet.value",
            false,
        ),
    ] {
        check(
            &format!(
                "data Packet [copy] {{ value: u8; }} machine produce() -> u8 ensures result == 7 {{ {body} }}"
            ),
            accepted,
        );
    }
}

#[test]
fn effectful_local_initializers_cannot_capture_post_call_operand_values() {
    for (initial, initializer, wrong_result) in [
        ("true", "flag && clear(&mut flag)", "false"),
        ("false", "flag || set(&mut flag)", "true"),
    ] {
        check(
            &format!(
                r#"
            machine clear(flag: &mut bool) -> bool {{ flag = false; true }}
            machine set(flag: &mut bool) -> bool {{ flag = true; false }}
            machine produce() -> bool ensures result == {wrong_result} {{
                let flag: bool = {initial};
                let saved: bool = {initializer};
                saved
            }}
        "#
            ),
            false,
        );
    }
}
