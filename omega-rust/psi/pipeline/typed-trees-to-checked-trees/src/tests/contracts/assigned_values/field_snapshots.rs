use super::*;

const DEFINITIONS: &str = r#"
    domain [u8; 2]::Utf8 requires valid_utf8(self);
    data Formatter { buffer: [u8; 2]; value: i32; digit: i32; }
    machine narrow(value: i32) -> u8 { (value as u8 in Trapping) as u8 }
    machine corrupt(value: &mut i32) { value = 200; }
    machine corrupt_byte(value: &mut u8) { value = 200; }
"#;

fn check_snapshot(source: &str, accepted: bool) {
    match lower_typed_trees(parse_typed_trees(source)) {
        Ok(_) => assert!(accepted, "unproved byte accepted:\n{source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{source}\n{diagnostics:#?}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
                "expected a failed output proof:\n{source}\n{diagnostics:#?}"
            );
        }
    }
}

#[test]
fn field_parameter_positions_do_not_alias_dense_scalar_bindings() {
    for parameters in [
        "formatter: &mut Formatter, prefix: i32, suffix: i32",
        "prefix: i32, formatter: &mut Formatter, suffix: i32",
        "prefix: i32, suffix: i32, formatter: &mut Formatter",
    ] {
        let source = format!(
            r#"{DEFINITIONS}
            machine format({parameters}) ensures formatter.buffer in Utf8 {{
                formatter.buffer = "AB";
                formatter.value = 25;
                let divisor: i32 = 10;
                formatter.digit = formatter.value / divisor;
                let offset: i32 = 48;
                formatter.digit = formatter.digit + offset;
                let byte: u8 = narrow(formatter.digit);
                formatter.buffer[0] = byte;
            }}
            "#
        );
        check_snapshot(&source, true);
    }
}

#[test]
fn captured_field_bytes_survive_source_writes_but_not_snapshot_writes() {
    for (after_capture, accepted) in [
        ("formatter.digit = 200;", true),
        ("corrupt(&mut formatter.digit);", true),
        ("byte = 200;", false),
        ("corrupt_byte(&mut byte);", false),
        (
            "formatter.digit = 200; byte = narrow(formatter.digit);",
            false,
        ),
    ] {
        let source = format!(
            r#"{DEFINITIONS}
            machine format(formatter: &mut Formatter)
            ensures formatter.buffer in Utf8 {{
                formatter.buffer = "AB";
                formatter.digit = 65;
                let mut byte: u8 = narrow(formatter.digit);
                {after_capture}
                formatter.buffer[0] = byte;
            }}
            "#
        );
        check_snapshot(&source, accepted);
    }
}

#[test]
fn equal_field_declarations_do_not_share_facts_between_record_instances() {
    for (prepare_right, selected, accepted) in [
        ("right.digit = 200;", "left.digit", true),
        ("right.digit = 200;", "right.digit", false),
        ("right.digit = unknown;", "right.digit", false),
        ("", "right.digit", false),
        (
            "right.digit = 65; corrupt(&mut right.digit);",
            "right.digit",
            false,
        ),
    ] {
        let source = format!(
            r#"{DEFINITIONS}
            machine format(left: &mut Formatter, unknown: i32, right: &mut Formatter)
            ensures left.buffer in Utf8 {{
                left.buffer = "AB";
                left.digit = 65;
                {prepare_right}
                let byte: u8 = narrow({selected});
                left.buffer[0] = byte;
            }}
            "#
        );
        check_snapshot(&source, accepted);
    }
}

#[test]
fn nested_field_paths_retain_the_exact_owner_and_each_member() {
    for (selected, accepted) in [("outer.payload.digit", true), ("outer.other.digit", false)] {
        let source = format!(
            r#"{DEFINITIONS}
            data Outer {{ payload: Formatter; other: Formatter; }}
            machine format(prefix: i32, outer: &mut Outer, suffix: i32)
            ensures outer.payload.buffer in Utf8 {{
                outer.payload.buffer = "AB";
                outer.payload.value = 25;
                outer.payload.digit = outer.payload.value / 10;
                outer.payload.digit = outer.payload.digit + 48;
                let byte: u8 = narrow({selected});
                outer.payload.digit = 200;
                outer.payload.buffer[0] = byte;
            }}
            "#
        );
        check_snapshot(&source, accepted);
    }
}
