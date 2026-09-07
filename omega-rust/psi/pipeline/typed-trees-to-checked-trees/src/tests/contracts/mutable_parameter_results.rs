use super::*;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(parse_typed_trees(source)) {
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
fn mixed_formals_and_locals_preserve_integer_binding_positions() {
    for (parameters, arguments) in [
        (
            "first: u8, mut current: u8, last: u8, mut other: u8",
            "1, 3, 11, 7",
        ),
        (
            "mut current: u8, first: u8, mut other: u8, last: u8",
            "3, 1, 7, 11",
        ),
    ] {
        for (returned, expected) in [
            ("first", 1),
            ("last", 11),
            ("current", 1),
            ("other", 11),
            ("initial", 3),
            ("saved", 7),
            ("scratch", 1),
        ] {
            for accepted in [true, false] {
                let comparison = if accepted { "==" } else { "!=" };
                check(
                    &format!(
                        r#"
                        machine rewrite({parameters}) -> u8 {{
                            let initial: u8 = current;
                            let mut scratch: u8 = other;
                            let retained: u8 = last;
                            current = scratch;
                            other = retained;
                            let saved: u8 = current;
                            scratch = first;
                            current = scratch;
                            {returned}
                        }}
                        machine caller() -> u8 ensures result {comparison} {expected} {{
                            let captured: u8 = rewrite({arguments});
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
fn mixed_formals_and_locals_preserve_boolean_binding_positions() {
    for (returned, expected) in [
        ("first", "false"),
        ("last", "true"),
        ("current", "false"),
        ("other", "true"),
        ("initial", "true"),
        ("saved", "false"),
        ("scratch", "true"),
    ] {
        for accepted in [true, false] {
            let comparison = if accepted { "==" } else { "!=" };
            check(
                &format!(
                    r#"
                    machine rewrite(mut current: bool, first: bool, mut other: bool, last: bool) -> bool {{
                        let initial: bool = current;
                        let mut scratch: bool = other;
                        let retained: bool = last;
                        current = scratch;
                        other = retained;
                        let saved: bool = current;
                        scratch = initial;
                        current = first;
                        {returned}
                    }}
                    machine caller() -> bool ensures result {comparison} {expected} {{
                        let captured: bool = rewrite(true, false, false, true);
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
fn unknown_parameter_results_do_not_acquire_known_caller_guarantees() {
    for (carrier, expected) in [("u8", "3"), ("bool", "true")] {
        for body in [
            "value".to_owned(),
            format!("let saved: {carrier} = value; value = value; saved"),
        ] {
            for comparison in ["==", "!="] {
                check(
                    &format!(
                        r#"
                        machine rewrite(mut value: {carrier}) -> {carrier} {{ {body} }}
                        machine caller(unknown: {carrier}) -> {carrier}
                        ensures result {comparison} {expected}
                        {{
                            let captured: {carrier} = rewrite(unknown);
                            captured
                        }}
                        "#,
                    ),
                    false,
                );
            }
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

#[test]
fn nonlocal_writes_do_not_publish_mutable_parameter_results() {
    check(
        r#"
        machine rewrite(mut value: u8, destination: &mut u8) -> u8 {
            destination = 9;
            value = value;
            value
        }
        machine caller() -> u8 ensures result == 3 {
            let mut destination: u8 = 0;
            let captured: u8 = rewrite(3, &mut destination);
            captured
        }
        "#,
        false,
    );
}

#[test]
fn owned_parameter_reassignment_preserves_caller_input_facts() {
    check(
        r#"
        machine rewrite(mut value: u8) -> u8 {
            value = 9;
            value
        }
        machine caller() -> u8 ensures result == 3 {
            let mut input: u8 = 3;
            let ignored: u8 = rewrite(input);
            input
        }
        "#,
        true,
    );
}

#[test]
fn nested_borrow_of_owned_parameter_preserves_caller_input_facts() {
    check(
        r#"
        machine corrupt(value: &mut u8) { value = 9; }
        machine rewrite(mut value: u8) -> u8 {
            corrupt(&mut value);
            value
        }
        machine caller() -> u8 ensures result == 3 {
            let mut input: u8 = 3;
            let ignored: u8 = rewrite(input);
            input
        }
        "#,
        true,
    );
}

#[test]
fn borrowed_parameter_reassignment_invalidates_caller_input_facts() {
    check(
        r#"
        machine rewrite(value: &mut u8) -> u8 {
            value = 9;
            9
        }
        machine caller() -> u8 ensures result == 3 {
            let mut input: u8 = 3;
            let ignored: u8 = rewrite(&mut input);
            input
        }
        "#,
        false,
    );
}
