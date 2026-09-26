use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()) {
        Ok(_) => assert!(
            accepted,
            "unproved mutable parameter result accepted:\n{source}"
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
fn fixed_integer_parameter_results_distinguish_current_and_saved_values() {
    for carrier in ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"] {
        for (returned, expected) in [("initial", 3), ("saved", 5), ("value", 9)] {
            for accepted in [true, false] {
                let comparison = if accepted { "==" } else { "!=" };
                check(
                    &format!(
                        r#"
                        machine rewrite(mut value: {carrier}) -> {carrier} {{
                            let initial: {carrier} = value;
                            value = value ^ 6{carrier};
                            let saved: {carrier} = value;
                            value = value ^ 12{carrier};
                            {returned}
                        }}
                        machine caller() -> {carrier}
                        ensures result {comparison} {expected}{carrier}
                        {{
                            let mut input: {carrier} = 3;
                            let captured: {carrier} = rewrite(input);
                            input = 15;
                            captured
                        }}
                        "#,
                    ),
                    accepted,
                );
            }
        }
    }
}

#[test]
fn boolean_parameter_results_distinguish_current_and_saved_values() {
    for (returned, expected) in [("initial", "false"), ("saved", "true"), ("value", "false")] {
        for accepted in [true, false] {
            let comparison = if accepted { "==" } else { "!=" };
            check(
                &format!(
                    r#"
                    machine rewrite(mut value: bool) -> bool {{
                        let initial: bool = value;
                        value = !value;
                        let saved: bool = value;
                        value = !value;
                        {returned}
                    }}
                    machine caller() -> bool ensures result {comparison} {expected} {{
                        let mut input: bool = false;
                        let captured: bool = rewrite(input);
                        input = !captured;
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
fn borrowed_corruption_does_not_publish_mutable_parameter_results() {
    for body in [
        "let alias: &mut u8 = &mut value; alias = 9; value",
        "corrupt(&mut value); value",
    ] {
        check(
            &format!(
                r#"
                machine corrupt(value: &mut u8) {{ value = 9; }}
                machine rewrite(mut value: u8) -> u8 {{ {body} }}
                machine caller() -> u8 ensures result == 3 {{
                    let captured: u8 = rewrite(3);
                    captured
                }}
                "#,
            ),
            false,
        );
    }
}
