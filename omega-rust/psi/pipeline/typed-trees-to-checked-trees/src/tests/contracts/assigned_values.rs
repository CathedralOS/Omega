use super::*;

#[test]
fn assignment_values_prove_nested_domain_outputs_and_preserve_disjoint_fields() {
    for body in [
        "player.health = 40; player.mana = 5;",
        "player.health = 40; player.mana = 5; player.tag = 200;",
    ] {
        let source = format!(
            r#"
            data Player {{ health: i32; mana: i32; tag: i32; }}
            domain Player::Valid requires self.health >= 0; self.health <= 100;
            domain Player::Ready requires self in Player::Valid; self.mana >= 0;
            machine establish(player: &mut Player) ensures player in Player::Ready {{ {body} }}
        "#
        );
        lower_typed_trees(parse_typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{body}: {diagnostics:#?}"));
    }
}

#[test]
fn assignment_values_cannot_survive_overlapping_or_unknown_writes() {
    for body in [
        "player.health = 40; player.health = 200;",
        "player.health = 40; let alias: &mut i32 = &mut player.health; alias = 200;",
        "player.health = 40; corrupt(&mut player.health);",
        "player.health = unknown;",
    ] {
        let source = format!(
            r#"
            data Player {{ health: i32; tag: i32; }}
            domain Player::Valid requires self.health >= 0; self.health <= 100;
            machine corrupt(value: &mut i32) {{ value = 200; }}
            machine establish(player: &mut Player, unknown: i32)
            ensures player in Player::Valid {{ {body} }}
        "#
        );
        let diagnostics = lower_typed_trees(parse_typed_trees(&source)).expect_err(body);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{body}: {diagnostics:#?}"
        );
    }
}

#[test]
fn assignment_values_do_not_grant_text_after_an_alias_or_index_write() {
    for body in [
        "line = \"okay\"; line[0] = 255;",
        "line = \"okay\"; let alias: &mut [u8; 4] = &mut line; alias[0] = 255;",
        "line = \"okay\"; corrupt(line);",
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            machine corrupt(line: &mut [u8; 4]) {{ line[0] = 255; }}
            machine establish(line: &mut [u8; 4]) ensures line in Utf8 {{ {body} }}
        "#
        );
        let diagnostics = lower_typed_trees(parse_typed_trees(&source)).expect_err(body);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{body}: {diagnostics:#?}"
        );
    }
}

#[test]
fn ascii_byte_replacement_does_not_preserve_arbitrary_utf8() {
    for index in [0, 1] {
        let source = format!(
            r#"
            domain [u8; 2]::Utf8 requires valid_utf8(self);
            machine invalid(line: &mut [u8; 2]) ensures line in Utf8 {{
                line = "é";
                line[{index}] = 65;
            }}
        "#
        );
        assert!(
            lower_typed_trees(parse_typed_trees(&source)).is_err(),
            "ASCII replacement may split a multibyte UTF-8 scalar"
        );
    }
}

#[test]
fn ascii_byte_replacement_preserves_a_whole_ascii_carrier() {
    for body in [
        // One store, then a second that must re-prove the class from the class
        // the first store left behind rather than from the retired literal.
        "line = \"AB\"; line[0] = 67;",
        "line = \"AB\"; line[0] = 67; line[1] = 68;",
    ] {
        let source = format!(
            r#"
            domain [u8; 2]::Utf8 requires valid_utf8(self);
            machine establish(line: &mut [u8; 2]) ensures line in Utf8 {{ {body} }}
        "#
        );
        let lowered = lower_typed_trees(parse_typed_trees(&source));
        assert!(
            lowered.is_ok(),
            "{body}: an ASCII byte written over an all-ASCII carrier keeps it valid UTF-8: {:#?}",
            lowered.err()
        );
    }
}

#[test]
fn non_ascii_replacement_retires_the_carrier_class() {
    for body in [
        // The replacement byte is not ASCII, so nothing survives the store.
        "line = \"AB\"; line[0] = 200;",
        // The class must die at the SECOND store even though the first one
        // legitimately re-established it.
        "line = \"AB\"; line[0] = 65; line[1] = 200;",
        // The carrier is valid UTF-8 but not ASCII, so no per-byte class holds
        // of it before the store and an ASCII byte may split its scalar.
        "line = \"\u{e9}\"; line[0] = 65;",
    ] {
        let source = format!(
            r#"
            domain [u8; 2]::Utf8 requires valid_utf8(self);
            machine establish(line: &mut [u8; 2]) ensures line in Utf8 {{ {body} }}
        "#
        );
        assert!(
            lower_typed_trees(parse_typed_trees(&source)).is_err(),
            "{body}: a byte outside the carrier's proved class cannot preserve it"
        );
    }
}

#[test]
fn selected_scalar_call_result_preserves_byte_class() {
    let source = r#"
        domain [u8; 2]::Utf8 requires valid_utf8(self);
        machine narrow(value: i32 [0..=255]) -> u8 { value as u8 }
        machine establish(line: &mut [u8; 2]) ensures line in Utf8 {
            let mut value: i32 = 25;
            value = value / 10;
            value = value + 48;
            let byte: u8 = narrow(value);
            value = 200;
            line = "AB";
            line[0] = byte;
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn selected_scalar_call_result_evaluates_immutable_locals() {
    for callee in [
        "let byte: u8 = value as u8; byte",
        "let byte: u8 = value as u8; let half: u8 = byte / 2; half",
    ] {
        let source = format!(
            r#"
            domain [u8; 2]::Utf8 requires valid_utf8(self);
            machine narrow(value: i32 [0..=255]) -> u8 {{ {callee} }}
            machine establish(line: &mut [u8; 2]) ensures line in Utf8 {{
                let mut value: i32 = 65;
                let byte: u8 = narrow(value);
                value = 200;
                line = "AB";
                line[0] = byte;
                line[1] = narrow(66);
            }}
        "#
        );
        lower_typed_trees(parse_typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{callee}: {diagnostics:#?}"));
    }
}

#[test]
fn selected_scalar_call_result_captures_projected_assignment() {
    let source = r#"
        data Holder { input: i32; byte: u8; }
        domain [u8; 2]::Utf8 requires valid_utf8(self);
        machine narrow(value: i32 [0..=255]) -> u8 { value as u8 }
        machine establish(line: &mut [u8; 2], holder: &mut Holder) ensures line in Utf8 {
            holder.input = 65;
            holder.byte = narrow(holder.input);
            holder.input = 200;
            line = "AB";
            line[0] = holder.byte;
            line[1] = narrow(66);
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn selected_scalar_call_result_evaluates_local_storage_in_order() {
    let source = r#"
        domain [u8; 2]::Utf8 requires valid_utf8(self);
        machine narrow(value: i32 [0..=255]) -> u8 {
            let mut byte: u8 = value as u8;
            byte = byte / 2;
            let saved: u8 = byte;
            byte = 200;
            saved
        }
        machine establish(line: &mut [u8; 2]) ensures line in Utf8 {
            let mut value: i32 = 130;
            let byte: u8 = narrow(value);
            value = 200;
            line = "AB";
            line[0] = byte;
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn trapping_conversion_snapshots_survive_nonwriting_unit_calls() {
    let source = r#"
        domain [u8; 2]::Utf8 requires valid_utf8(self);
        machine observe(value: bool) {}
        machine narrow(value: i32) -> u8 {
            let below: bool = value < 0;
            observe(below);
            let above: bool = value > 255;
            observe(above);
            (value as u8 in Trapping) as u8
        }
        machine establish(line: &mut [u8; 2]) ensures line in Utf8 {
            let mut value: i32 = 65;
            let byte: u8 = narrow(value);
            value = 200;
            line = "AB";
            line[0] = byte;
            line[1] = narrow(66);
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn normal_return_snapshots_do_not_assume_a_safe_conversion_or_carrier() {
    for (input, initial, after_capture) in [
        ("-1", "AB", ""),
        ("256", "AB", ""),
        ("200", "AB", ""),
        ("unknown", "AB", ""),
        ("65", "é", ""),
        ("65", "AB", "byte = 200;"),
        ("65", "AB", "corrupt(&mut byte);"),
    ] {
        let source = format!(
            r#"
            domain [u8; 2]::Utf8 requires valid_utf8(self);
            machine observe(value: bool) {{}}
            machine corrupt(value: &mut u8) {{ value = 200; }}
            machine narrow(value: i32) -> u8 {{
                observe(false);
                (value as u8 in Trapping) as u8
            }}
            machine establish(line: &mut [u8; 2], unknown: i32) ensures line in Utf8 {{
                let mut byte: u8 = narrow({input});
                {after_capture}
                line = "{initial}";
                line[0] = byte;
            }}
        "#
        );
        let diagnostics = lower_typed_trees(parse_typed_trees(&source)).expect_err(&source);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn numeric_text_writer_captures_field_computations_before_conversion() {
    let source = r#"
        domain [u8; 2]::Utf8 requires valid_utf8(self);
        data Formatter { buffer: [u8; 2]; value: i32; digit: i32; byte: u8; }
        machine observe(invalid: bool) {
            let invalid_value: u32 in Wrapping = invalid as u32 in Wrapping;
            let count: u32 in Wrapping = invalid_value * 8;
            let probe: u8 in Trapping = (1 as u8 in Trapping) << (count as u32 in Trapping);
        }
        machine narrow(value: i32) -> u8 {
            let below: bool = value < 0;
            observe(below);
            let above: bool = value > 255;
            observe(above);
            (value as u8 in Trapping) as u8
        }
        machine format(formatter: &mut Formatter) ensures formatter.buffer in Utf8 {
            formatter.buffer = "AB";
            formatter.value = 25;
            formatter.digit = formatter.value / 10;
            formatter.digit = formatter.digit + 48;
            formatter.byte = narrow(formatter.digit);
            formatter.buffer[0] = formatter.byte;
        }
    "#;
    for source in [
        source.to_owned(),
        source
            .replace(
                "machine format(formatter: &mut Formatter)",
                "machine Formatter::format(&mut self)",
            )
            .replace("formatter.", "self."),
    ] {
        lower_typed_trees(parse_typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    }
}

#[test]
fn scalar_snapshots_reject_borrowed_calls_and_effectful_arguments() {
    for (declaration, invocation) in [
        (
            "machine observe(value: &mut u8) { value = 200; }",
            "observe(&mut byte);",
        ),
        (
            "machine observe(value: bool) {}",
            "observe(change(&mut byte));",
        ),
    ] {
        let source = format!(
            r#"
            domain [u8; 2]::Utf8 requires valid_utf8(self);
            {declaration}
            machine change(value: &mut u8) -> bool {{ value = 200; false }}
            machine narrow() -> u8 {{
                let mut byte: u8 = 65;
                {invocation}
                byte
            }}
            machine establish(line: &mut [u8; 2]) ensures line in Utf8 {{
                line = "AB";
                line[0] = narrow();
            }}
        "#
        );
        let diagnostics = lower_typed_trees(parse_typed_trees(&source)).expect_err(&source);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn selected_scalar_call_result_rejects_unproved_or_replaced_bytes() {
    for (callee, body) in [
        ("value as u8", "let byte: u8 = narrow(200); line[0] = byte;"),
        ("200", "let byte: u8 = narrow(65); line[0] = byte;"),
        (
            "value as u8",
            "let byte: u8 = narrow(unknown); line[0] = byte;",
        ),
        (
            "value as u8",
            "let mut byte: u8 = narrow(65); byte = 200; line[0] = byte;",
        ),
        (
            "value as u8",
            "let mut byte: u8 = narrow(65); corrupt(&mut byte); line[0] = byte;",
        ),
        (
            "let byte: u8 = value as u8; let replaced: u8 = 200; replaced",
            "let byte: u8 = narrow(65); line[0] = byte;",
        ),
        (
            "let byte: u8 = value as u8; byte",
            "let byte: u8 = narrow(200); line[0] = byte;",
        ),
        (
            "let byte: u8 = value as u8; byte",
            "let byte: u8 = narrow(unknown); line[0] = byte;",
        ),
        (
            "let ignored: u8 = other(); let byte: u8 = value as u8; byte",
            "let byte: u8 = narrow(65); line[0] = byte;",
        ),
        (
            "let mut byte: u8 = 65; corrupt(&mut byte); byte",
            "let byte: u8 = narrow(65); line[0] = byte;",
        ),
        (
            "value as u8; state other(value: i32 [0..=255]) -> u8 { value as u8 }",
            "let byte: u8 = narrow(65); line[0] = byte;",
        ),
    ] {
        let source = format!(
            r#"
            domain [u8; 2]::Utf8 requires valid_utf8(self);
            machine narrow(value: i32 [0..=255]) -> u8 {{ {callee} }}
            machine corrupt(value: &mut u8) {{ value = 200; }}
            machine other() -> u8 {{ 200 }}
            machine establish(line: &mut [u8; 2], unknown: i32 [0..=255])
            ensures line in Utf8 {{ line = "AB"; {body} }}
        "#
        );
        let diagnostics = lower_typed_trees(parse_typed_trees(&source)).expect_err(body);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{callee}: {body}: {diagnostics:#?}"
        );
    }
}
mod field_snapshots;
