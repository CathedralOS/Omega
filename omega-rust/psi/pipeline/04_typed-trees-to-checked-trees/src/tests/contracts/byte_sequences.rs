use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()) {
        Ok(_) => assert!(accepted, "unproved indexed byte write accepted:\n{source}"),
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
fn copied_byte_predicates_follow_materialized_storage() {
    for (body, succeeds) in [
        (
            "input.bytes[position] = 65; output.bytes = input.bytes;",
            true,
        ),
        (
            "input.bytes[position] = 65; let saved: [u8; 4] = input.bytes; output.bytes = saved;",
            true,
        ),
        (
            "input.bytes[position] = 65; output.bytes = input.bytes; input.bytes[position] = 255;",
            true,
        ),
        (
            "input.bytes[position] = 255; output.bytes = input.bytes;",
            false,
        ),
        (
            "input.bytes[position] = 65; output.bytes = input.bytes; output.bytes[position] = 66;",
            true,
        ),
        (
            "input.bytes[position] = 65; output.bytes = input.bytes; let alias: &mut [u8; 4] = &mut output.bytes; alias[position] = 255;",
            false,
        ),
        (
            "input.bytes[position] = 65; output.bytes = input.bytes; output.bytes[position] = 255;",
            false,
        ),
        (
            "input.bytes[position] = 65; output.bytes = input.bytes; corrupt(&mut output.bytes);",
            false,
        ),
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Ascii requires ascii_only(self);
            data Buffer {{ bytes: [u8; 4]; }}
            machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
            machine copy(output: &mut Buffer, input: &mut Buffer, position: u64 [0..=3])
            requires input.bytes in Ascii
            ensures output.bytes in Ascii {{ {body} }}
            "#
        );
        let result = lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled());
        assert_eq!(result.is_ok(), succeeds, "{body}: {result:#?}");
        if let Err(diagnostics) = result {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
                "{body}: {diagnostics:#?}"
            );
        }
    }
}

const DEFINITIONS: &str = r#"
domain [u8; 4]::Utf8 requires valid_utf8(self);
domain [u8; 16]::Utf8 requires valid_utf8(self);
data Input { bytes: [u8; 4]; }
"#;

#[test]
fn concatenated_output_predicates_are_retired_by_destination_mutations() {
    for mutation in [
        "output[0] = 255;",
        "let alias: &mut [u8; 16] = &mut output; alias[0] = 255;",
        "corrupt(output);",
    ] {
        let source = format!(
            r#"{DEFINITIONS}
            machine corrupt(bytes: &mut [u8; 16]) {{ bytes[0] = 255; }}
            machine concatenate(output: &mut [u8; 16], input: Input)
            requires input.bytes in Utf8
            ensures output in Utf8 {{ output = input.bytes + "!"; {mutation} }}
            "#
        );
        let diagnostics =
            lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
                .expect_err(mutation);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{mutation}: {diagnostics:#?}"
        );
    }
}

#[test]
fn converted_call_results_carry_their_conversion_bounds() {
    // A selected call nested inside authored value conversions is still a
    // selected call: each conversion's own law transforms the callee's
    // captured result range, and the indexed write consumes the converted
    // range. A conversion that cannot describe a normal-return byte, and a
    // callee whose captured result escapes it, both fail closed.
    for (definitions, statement, succeeds) in [
        // Wrapping keeps a captured interval that already fits the carrier.
        (
            "machine digit(value: u64) -> u64 { value % 10 + 48 }",
            "output[position] = ((digit(unknown) as u8 in Wrapping) as u8);",
            true,
        ),
        // An interval inside the carrier but outside the predicate, and one
        // that can actually wrap to the full carrier, both reject.
        (
            "machine digit(value: u64) -> u64 { value % 200 + 48 }",
            "output[position] = ((digit(unknown) as u8 in Wrapping) as u8);",
            false,
        ),
        (
            "machine digit(value: u64) -> u64 { value % 300 + 48 }",
            "output[position] = ((digit(unknown) as u8 in Wrapping) as u8);",
            false,
        ),
        // Trapping keeps the representable meet; a result that need not be
        // representable describes no normal-return byte.
        (
            "machine digit(value: u64) -> u64 { value % 10 + 48 }",
            "output[position] = ((digit(unknown) as u8 in Trapping) as u8);",
            true,
        ),
        (
            "machine digit(value: u64) -> u64 { value }",
            "output[position] = ((digit(unknown) as u8 in Trapping) as u8);",
            false,
        ),
        // Saturating clamps the captured interval into the carrier.
        (
            "machine digit(value: u64) -> u64 { value % 10 + 48 }",
            "output[position] = ((digit(unknown) as u8 in Saturating) as u8);",
            true,
        ),
        (
            "machine digit(value: u64) -> u64 { value % 200 + 48 }",
            "output[position] = ((digit(unknown) as u8 in Saturating) as u8);",
            false,
        ),
        // An effect-free nested Unit call inside the callee stays
        // transparent; a real write footprint still retires the capture.
        (
            "machine observe(value: u64) {}
             machine digit(value: u64) -> u64 { observe(value + 1); value % 10 + 48 }",
            "output[position] = ((digit(unknown) as u8 in Wrapping) as u8);",
            true,
        ),
        (
            "machine poke(cell: &mut u64) { cell = 255; }
             machine digit(value: u64) -> u64 { let mut scratch: u64 = 0; poke(&mut scratch); value % 10 + 48 }",
            "output[position] = ((digit(unknown) as u8 in Wrapping) as u8);",
            false,
        ),
        // A landed local keeps the converted bounds, and a same-carrier retag
        // of a byte callee stays transparent.
        (
            "machine digit(value: u64) -> u64 { value % 10 + 48 }",
            "let saved: u8 = ((digit(unknown) as u8 in Wrapping) as u8); output[position] = saved;",
            true,
        ),
        (
            "machine digit(value: u64) -> u8 { ((value % 10 + 48) as u8 in Wrapping) as u8 }",
            "output[position] = (digit(unknown) as u8);",
            true,
        ),
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Ascii requires ascii_only(self);
            {definitions}
            machine write(output: &mut [u8; 4], position: u64 [0..=3], unknown: u64)
            requires output in Ascii
            ensures output in Ascii {{ {statement} }}
            "#
        );
        check(&source, succeeds);
    }
}

#[test]
fn concatenation_uses_the_shared_predicate_law_without_domain_names() {
    for (predicate, literal, succeeds) in [
        ("ascii_only", "ascii", true),
        ("ascii_only", "é", false),
        ("no_nul", "text", true),
        ("non_empty", "text", true),
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::InputProperty requires {predicate}(self);
            domain [u8; 16]::OutputProperty requires {predicate}(self);
            machine concatenate(output: &mut [u8; 16], input: [u8; 4])
            requires input in InputProperty
            ensures output in OutputProperty {{ output = input + "{literal}"; }}
            "#
        );
        let result = lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled());
        assert_eq!(
            result.is_ok(),
            succeeds,
            "{predicate}/{literal}: {result:#?}"
        );
    }
}
