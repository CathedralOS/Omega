use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use checked_interpreter::interpret_entry;

fn execute(source: &str) -> checked_interpreter::InterpretOutcome {
    let checked = crate::front_end::checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    )
}

#[test]
fn integer_execution_consumes_exact_decimal_landings() {
    for (expression, expected) in [
        ("7 / 2.0 * 2", 7),
        ("0.1 + 0.9", 1),
        ("9007199254740993.0 - 9007199254740992", 1),
        ("7i32 / 2 * 2", 6),
    ] {
        for body in [
            expression.to_owned(),
            format!("let saved: i32 = {expression}; saved"),
            format!("let mut saved: i32 = 0; saved = {expression}; saved"),
            format!("accept({expression})"),
            format!("({expression}) as i32"),
        ] {
            let source = format!(
                "machine accept(value: i32) -> i32 {{ value }}
                 machine main() -> i32 {{ {body} }}"
            );
            let outcome = execute(&source);
            assert_eq!(outcome.error, None, "{source}");
            assert_eq!(outcome.exit_code, expected, "{source}");
        }
    }
}

#[test]
fn selected_case_uses_its_own_integer_field_destination() {
    for expression in ["7 / 2 * 2", "7 / 2.0 * 2", "0.1 * 70"] {
        let source = format!(
            "data Number {{ case Floating(value: f64); case Integral(value: i32); }}
             machine main() -> i32 {{
                 let number: Number = Number::Integral {{ value: {expression} }};
                 transition number {{
                     Number::Floating {{ value }} -> 0
                     Number::Integral {{ value }} -> finish(value)
                 }}
                 state finish(value: i32) -> i32 {{ value }}
             }}"
        );
        let outcome = execute(&source);
        assert_eq!(outcome.error, None, "{source}");
        assert_eq!(outcome.exit_code, 7, "{source}");
    }
}

#[test]
fn integer_peers_and_state_edges_share_exact_decimal_values() {
    for expression in [
        "input * (9007199254740993.0 - 9007199254740992)",
        "(0.1 + 0.9) * input",
    ] {
        let source = format!(
            "machine combine(input: i32 [0..=1]) -> i32 {{ {expression} }}
             machine main() -> i32 {{ combine(1) }}"
        );
        let outcome = execute(&source);
        assert_eq!(outcome.error, None, "{source}");
        assert_eq!(outcome.exit_code, 1, "{source}");
    }
    for source in [
        "machine main() -> i32 { transition true { true -> 7 / 2.0 * 2 false -> 0 } }",
        "machine main() -> i32 { transition { _ -> finish(7 / 2.0 * 2) } state finish(value: i32) -> i32 { value } }",
    ] {
        let outcome = execute(source);
        assert_eq!(outcome.error, None, "{source}");
        assert_eq!(outcome.exit_code, 7, "{source}");
    }
}

#[test]
fn exact_argument_landing_preserves_forwarded_mutable_integer_references() {
    let outcome = execute(
        "machine assign(value: &mut i32) { value = 7 / 2.0 * 2; }
         machine forward(value: &mut i32) { assign(value); }
         machine main() -> i32 { let mut value: i32 = 0; forward(&mut value); value }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn casts_retain_high_unsigned_literal_and_computed_carriers() {
    for expression in [
        "18446744073709551615u64",
        "18446744073709551615",
        "18446744073709551614u64 + 1u64",
        "match true { true -> 18446744073709551615, false -> 0 }",
    ] {
        let source = format!(
            "machine main() -> i32 {{
                let value: u64 = ({expression}) as u64;
                transition value == 18446744073709551615u64 {{ true -> 70 false -> 71 }}
            }}"
        );
        let outcome = execute(&source);
        assert_eq!(outcome.error, None, "{source}");
        assert_eq!(outcome.exit_code, 70, "{source}");
    }
}

#[test]
fn trapping_address_conversion_checks_dynamic_input_before_narrowing() {
    for (input, expected) in [(88_u64, Some(88)), (2147483648, None), (u64::MAX, None)] {
        let source = format!(
            "machine convert(input: addr) -> i32 {{ (input as i32 in Trapping) as i32 }}
             machine main() -> i32 {{ let address: addr = {input}; convert(address) }}"
        );
        let outcome = execute(&source);
        if let Some(expected) = expected {
            assert_eq!(outcome.error, None, "{source}");
            assert_eq!(outcome.exit_code, expected, "{source}");
        } else {
            assert!(
                outcome
                    .error
                    .as_deref()
                    .is_some_and(|error| error
                        .contains("integer conversion out of range in Trapping domain")),
                "{source}: {:?}",
                outcome.error
            );
        }
    }
}

#[test]
fn casts_retain_unsigned_local_and_field_carriers() {
    for field in ["maximum", "context.maximum"] {
        let source = format!(
            "data Main {{ maximum: u64; }}
             machine Main::main() -> i32 {{
                 let maximum: u64 = 18446744073709551615;
                 let context: Main = Main {{ maximum: maximum }};
                 let value: u64 = {field} as u64;
                 transition value == 18446744073709551615u64 {{ true -> 70 false -> 71 }}
             }}"
        );
        let checked = crate::front_end::checked_program_result(&source)
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        let outcome = interpret_entry(
            &checked,
            BuildMachineEntry::Name("Main::main"),
            &[],
            InterpretOptions::default(),
        );
        assert_eq!(outcome.error, None, "{source}");
        assert_eq!(outcome.exit_code, 70, "{source}");
    }
}
