use super::*;

fn byte_store_source(callee: &str, body: &str) -> String {
    format!(
        r#"
        domain [u8; 2]::Ascii requires ascii_only(self);
        machine narrow(value: u8) -> u8 {{ {callee} }}
        machine corrupt(value: &mut u8) {{ value = 200; }}
        machine other() -> u8 {{ 200 }}
        machine establish(line: &mut [u8; 2], unknown: u8)
        ensures line in Ascii {{ line = "AB"; {body} }}
        "#,
    )
}

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(parse_typed_trees(source)) {
        Ok(_) => assert!(
            accepted,
            "unproved scalar storage result accepted:\n{source}"
        ),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
                "expected an unproved caller guarantee: {diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn current_storage_and_saved_local_results_preserve_ascii() {
    for callee in [
        "let mut byte: u8 = 200; byte = value; byte",
        "let mut byte: u8 = value; let saved: u8 = byte; byte = 200; saved",
    ] {
        check(
            &byte_store_source(
                callee,
                r#"
                let mut input: u8 = 65;
                let byte: u8 = narrow(input);
                input = 200;
                line[0] = byte;
                "#,
            ),
            true,
        );
    }
}

#[test]
fn interleaved_mutable_locals_keep_immutable_binding_ordinals() {
    for returned in ["left", "last"] {
        let callee = format!(
            r#"
            let first: u8 = value;
            let mut left: u8 = 200;
            let initial: u8 = left;
            let mut right: u8 = first;
            left = right;
            let saved: u8 = left;
            right = initial;
            let last: u8 = saved;
            {returned}
            "#,
        );
        check(
            &byte_store_source(&callee, "let byte: u8 = narrow(65); line[0] = byte;"),
            true,
        );
    }
}

#[test]
fn boolean_storage_results_materialize_caller_guarantees() {
    for (returned, expected) in [("flag", "false"), ("saved", "true")] {
        for accepted in [true, false] {
            let comparison = if accepted { "==" } else { "!=" };
            check(
                &format!(
                    r#"
                    machine choose(value: bool) -> bool {{
                        let mut flag: bool = value;
                        flag = !flag;
                        let saved: bool = flag;
                        flag = false;
                        {returned}
                    }}
                    machine caller() -> bool ensures result {comparison} {expected} {{
                        let mut input: bool = false;
                        let captured: bool = choose(input);
                        input = true;
                        captured
                    }}
                    "#,
                ),
                accepted,
            );
        }
    }
}

#[test]
fn final_non_ascii_storage_cannot_preserve_ascii() {
    check(
        &byte_store_source(
            "let mut byte: u8 = value; let saved: u8 = byte; byte = 200; byte",
            "let byte: u8 = narrow(65); line[0] = byte;",
        ),
        false,
    );
}

#[test]
fn later_ascii_storage_cannot_bless_a_saved_non_ascii_value() {
    check(
        &byte_store_source(
            "let mut byte: u8 = 200; let saved: u8 = byte; byte = value; saved",
            "let byte: u8 = narrow(65); line[0] = byte;",
        ),
        false,
    );
}

#[test]
fn unknown_and_corrupted_storage_results_remain_unproved() {
    for (callee, body) in [
        (
            "let mut byte: u8 = 65; byte = value; byte",
            "let byte: u8 = narrow(unknown); line[0] = byte;",
        ),
        (
            "let mut byte: u8 = value; let alias: &mut u8 = &mut byte; alias = 200; byte",
            "let byte: u8 = narrow(65); line[0] = byte;",
        ),
        (
            "let mut byte: u8 = value; corrupt(&mut byte); byte",
            "let byte: u8 = narrow(65); line[0] = byte;",
        ),
        (
            "let mut byte: u8 = value; byte = byte; byte",
            "let mut byte: u8 = narrow(65); byte = unknown; line[0] = byte;",
        ),
        (
            "let mut byte: u8 = value; byte = byte; byte",
            "let mut byte: u8 = narrow(65); let alias: &mut u8 = &mut byte; alias = 200; line[0] = byte;",
        ),
        (
            "let mut byte: u8 = value; byte = byte; byte",
            "let mut byte: u8 = narrow(65); corrupt(&mut byte); line[0] = byte;",
        ),
    ] {
        check(&byte_store_source(callee, body), false);
    }
}

#[test]
fn unsupported_extra_calls_do_not_publish_storage_snapshots() {
    for callee in [
        "let ignored: u8 = other(); let mut byte: u8 = value; byte",
        "let mut byte: u8 = value; let saved: u8 = byte; let ignored: u8 = other(); saved",
        "let mut byte: u8 = other(); byte = value; byte",
    ] {
        check(
            &byte_store_source(callee, "let byte: u8 = narrow(65); line[0] = byte;"),
            false,
        );
    }
}
