use super::check;

#[test]
fn storage_initializers_and_assignments_preserve_selected_results() {
    for (value_type, computation, expected) in [
        ("u8", "3 + 4", "7"),
        ("u8", "(255 + 1) - 1", "255"),
        (
            "u64",
            "(18446744073709551615 + 1) - 1",
            "18446744073709551615u64",
        ),
        ("i8", "(0 - 129) + 1", "-128"),
        ("u8", "((255u8 as u8 in Wrapping) + 2) as u8", "1"),
        ("u8", "((255u8 as u8 in Saturating) + 2) as u8", "255"),
        ("bool", "false || true", "true"),
    ] {
        let zero = if value_type == "bool" { "false" } else { "0" };
        for body in [
            format!("let mut stored: {value_type} = {computation}; stored"),
            format!("let mut stored: {value_type} = {zero}; stored = {computation}; stored"),
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
}

#[test]
fn stored_computations_use_live_immutable_operands_without_replaying_them() {
    for (body, expected) in [
        (
            "let input: u8 = 3; let mut stored: u8 = ((input as u8 in Wrapping) + 4) as u8; replace(&mut input); stored",
            7,
        ),
        (
            "let input: u8 = 3; let mut stored: u8 = 0; stored = ((input as u8 in Wrapping) + 4) as u8; replace(&mut input); stored",
            7,
        ),
        (
            "let mut stored: u8 = 3 + 4; let saved: u8 = stored; stored = 4 + 4; saved",
            7,
        ),
        (
            "let mut stored: u8 = 3 + 4; let saved: u8 = stored; stored = 4 + 4; stored",
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
}

#[test]
fn unknown_assignment_and_effectful_initializer_cannot_retain_old_values() {
    for body in [
        "let mut stored: u8 = 3 + 4; stored = unknown; stored",
        "let mut stored: u8 = 3 + 4; replace(&mut stored); stored",
        "let input: u8 = 3; replace(&mut input); let mut stored: u8 = ((input as u8 in Wrapping) + 4) as u8; stored",
        "let mut stored: u8 = unexplained(); stored",
    ] {
        check(
            &format!(
                "machine replace(value: &mut u8) {{ value = 8; }} machine unexplained() -> u8 {{ 8 }} machine produce(unknown: u8) -> u8 ensures result == 7 {{ {body} }}"
            ),
            false,
        );
    }
}

#[test]
fn computed_assignments_into_fields_and_state_arguments_keep_exact_results() {
    check(
        "machine produce(output: &mut u8) ensures output == 255 { output = (255 + 1) - 1; }",
        true,
    );
    check(
        r#"
        data Packet [copy] { value: u8; }
        machine produce() -> u8 ensures result == 255 {
            let mut packet: Packet = Packet { value: 0 };
            packet.value = (255 + 1) - 1;
            transition { _ -> finish(packet.value) }
            state finish(current: u8) -> u8 { current }
        }
    "#,
        true,
    );
}
