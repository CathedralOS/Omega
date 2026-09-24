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

#[test]
fn direct_computed_byte_stores_use_selected_arithmetic() {
    for (body, succeeds) in [
        ("output[position] = 1 / 2 * 400;", false),
        ("output[position] = 1 / 2 * 130;", true),
        ("let byte: u8 = 60; output[position] = byte + 5;", true),
        ("output[position] = 60 + 5;", true),
        ("output[position] = (250 as u8 in Wrapping) + 71;", true),
        ("output[position] = (250 as u8 in Saturating) + 71;", false),
        ("let byte: u8 = 120; output[position] = byte + 8;", false),
        (
            "let mut byte: u8 = 60; byte = unknown; output[position] = (byte as u8 in Wrapping) + 5;",
            false,
        ),
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Ascii requires ascii_only(self);
            machine replace(output: &mut [u8; 4], position: u64 [0..=3], unknown: u8)
            requires output in Ascii
            ensures output in Ascii {{ {body} }}
            "#
        );
        let result = lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled());
        assert_eq!(
            result.is_ok(),
            succeeds,
            "{body}: {:#?}",
            result.as_ref().err()
        );
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

#[test]
fn unknown_scalar_inputs_bound_conversion_results_by_their_carrier() {
    for (callee_body, statement, accepted) in [
        (
            "((value % 10 + 48) as u8 in Wrapping) as u8",
            "output[position] = digit(unknown);",
            true,
        ),
        (
            "((value % 200 + 48) as u8 in Wrapping) as u8",
            "output[position] = digit(unknown);",
            false,
        ),
        (
            "0",
            "output[position] = ((unknown % 10 + 48) as u8 in Wrapping) as u8;",
            true,
        ),
        (
            "0",
            "output[position] = ((unknown % 200 + 48) as u8 in Wrapping) as u8;",
            false,
        ),
        (
            "0",
            "let copy: u64 = unknown; output[position] = ((copy % 10 + 48) as u8 in Wrapping) as u8;",
            true,
        ),
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Ascii requires ascii_only(self);
            machine digit(value: u64) -> u8 {{ {callee_body} }}
            machine write(output: &mut [u8; 4], position: u64 [0..=3], unknown: u64)
            requires output in Ascii
            ensures output in Ascii {{ {statement} }}
            "#
        );
        check(&source, accepted);
    }
}

#[test]
fn computed_byte_stores_use_live_scalar_snapshots() {
    for (body, succeeds) in [
        ("let byte: u8 = 60 + 5; output[position] = byte;", true),
        (
            "let mut byte: u8 = 60; byte = byte + 5; output[position] = byte;",
            true,
        ),
        (
            "let mut byte: u8 = 60; let saved: u8 = byte + 5; byte = 255; output[position] = saved;",
            true,
        ),
        ("let byte: u8 = 120 + 8; output[position] = byte;", false),
        (
            "let mut byte: u8 = 60 + 5; byte = unknown; output[position] = byte;",
            false,
        ),
        (
            "let mut byte: u8 = 60 + 5; corrupt(&mut byte); output[position] = byte;",
            false,
        ),
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Ascii requires ascii_only(self);
            machine corrupt(byte: &mut u8) {{ byte = 255; }}
            machine replace(output: &mut [u8; 4], position: u64 [0..=3], unknown: u8)
            requires output in Ascii
            ensures output in Ascii {{ {body} }}
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

#[test]
fn dynamic_byte_stores_preserve_only_proved_carrier_predicates() {
    for (predicate, byte, succeeds) in [
        ("ascii_only", 65, true),
        ("ascii_only", 255, false),
        ("no_nul", 65, true),
        ("no_nul", 0, false),
        ("valid_utf8", 65, false),
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Property requires {predicate}(self);
            machine replace(output: &mut [u8; 4], position: u64 [0..=3])
            requires output in Property
            ensures output in Property {{
                output[position] = 65;
                output[position] = {byte};
            }}
            "#
        );
        let result = lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled());
        assert_eq!(result.is_ok(), succeeds, "{predicate}/{byte}: {result:#?}");
        if let Err(diagnostics) = result {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| { diagnostic.message.contains("cannot prove ensures") }),
                "{predicate}/{byte}: {diagnostics:#?}"
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
fn concatenation_establishes_a_raw_output_from_nested_live_operands() {
    let source = format!(
        r#"{DEFINITIONS}
        machine concatenate(output: &mut [u8; 16], input: Input)
        requires input.bytes in Utf8
        ensures output in Utf8 {{
            output = (input.bytes + "!") + "!";
        }}
        "#
    );
    lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
        .expect("nested concatenation establishes output");
}

#[test]
fn concatenation_requires_every_operand_to_have_a_live_predicate() {
    for body in [
        "output = input.bytes + unknown;",
        "input.bytes[0] = 255; output = input.bytes + \"!\";",
    ] {
        let source = format!(
            r#"{DEFINITIONS}
            machine concatenate(output: &mut [u8; 16], input: &mut Input, unknown: [u8; 4])
            requires input.bytes in Utf8
            ensures output in Utf8 {{ {body} }}
            "#
        );
        let diagnostics =
            lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
                .expect_err(body);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{body}: {diagnostics:#?}"
        );
    }
}

#[test]
fn concatenated_output_does_not_replay_a_later_source_mutation() {
    let source = format!(
        r#"{DEFINITIONS}
        machine concatenate(output: &mut [u8; 16], input: &mut Input)
        requires input.bytes in Utf8
        ensures output in Utf8 {{
            output = input.bytes + "!";
            input.bytes[0] = 255;
        }}
        "#
    );
    lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
        .expect("copied bytes are independent of input");
}

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
fn concatenation_reads_the_prewrite_value_for_an_inplace_append() {
    let source = format!(
        r#"{DEFINITIONS}
        machine concatenate(output: &mut [u8; 16])
        requires output in Utf8
        ensures output in Utf8 {{ output = output + "!"; }}
        "#
    );
    lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
        .expect("in-place append reads the old value");
}

#[test]
fn call_result_bounds_survive_a_local_landing() {
    // A completed call-result snapshot keeps its captured bounds at the
    // local's place; the retained call provenance must not mask them. An
    // opaque or wide call result still cannot prove the byte predicate.
    for (definitions, body, succeeds) in [
        (
            "machine digit(value: u64) -> u8 { ((value % 10 + 48) as u8 in Wrapping) as u8 }",
            "let saved: u8 = digit(unknown); output[position] = saved;",
            true,
        ),
        (
            "machine digit(value: u64) -> u8 { ((value % 10 + 48) as u8 in Wrapping) as u8 }",
            "let saved: u8 = digit(unknown); let copy: u8 = saved; output[position] = copy;",
            true,
        ),
        (
            "machine byte(value: u64) -> u8 { (value as u8 in Wrapping) as u8 }",
            "let saved: u8 = byte(unknown); output[position] = saved;",
            false,
        ),
        (
            "machine byte(scratch: &mut u64, value: u64) -> u8 { scratch = value; ((scratch % 10 + 48) as u8 in Wrapping) as u8 }",
            "let mut scratch: u64 = 0; let saved: u8 = byte(&mut scratch, unknown); output[position] = saved;",
            false,
        ),
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Ascii requires ascii_only(self);
            {definitions}
            machine write(output: &mut [u8; 4], position: u64 [0..=3], unknown: u64)
            requires output in Ascii
            ensures output in Ascii {{ {body} }}
            "#
        );
        check(&source, succeeds);
    }
}

#[test]
fn declared_ranges_bound_frozen_storage_reads() {
    // A declared range constraint is the storage invariant every checked
    // write enforced; it bounds reads of immutable parameters and their
    // fields even when no narrower value snapshot is live. This is carrier
    // evidence only -- never domain membership or initialization.
    for (data_decl, unknown_decl, body, succeeds) in [
        (
            "data Input { value: u64 [0..=9]; }",
            "u64",
            "output[position] = (input.value as u8 in Wrapping) + 48;",
            true,
        ),
        (
            "data Input { value: u64 [0..=9]; }",
            "u64 [0..=9]",
            "output[position] = (unknown as u8 in Wrapping) + 48;",
            true,
        ),
        (
            "data Input { value: u64 [0..=9]; }",
            "u64",
            "let byte: u64 [0..=9] = input.value; output[position] = (byte as u8 in Wrapping) + 48;",
            true,
        ),
        (
            "data Input { value: u64 [0..=90]; }",
            "u64",
            "output[position] = (input.value as u8 in Wrapping) + 48;",
            false,
        ),
        (
            "data Input { value: u64; }",
            "u64",
            "output[position] = (input.value as u8 in Wrapping) + 48;",
            false,
        ),
        (
            "data Input { value: u64 [0..=9]; }",
            "u64 [0..=200]",
            "output[position] = (unknown as u8 in Wrapping) + 48;",
            false,
        ),
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Ascii requires ascii_only(self);
            {data_decl}
            machine write(output: &mut [u8; 4], input: Input, position: u64 [0..=3], unknown: {unknown_decl})
            requires output in Ascii
            ensures output in Ascii {{ {body} }}
            "#
        );
        check(&source, succeeds);
    }
}

#[test]
fn declared_ranges_do_not_bound_mutably_borrowed_storage() {
    // A `mut` local or `&mut` parameter field can be reborrowed into a call
    // whose own pointee type drops the declared range, so the declared bound
    // is no read invariant there: after `corrupt`, only the carrier is known.
    for (input_decl, body) in [
        (
            "input: Input",
            "let mut local: u64 [0..=9] = 5; corrupt(&mut local); output[position] = (local as u8 in Wrapping) + 48;",
        ),
        (
            "input: &mut Input",
            "corrupt(&mut input.value); output[position] = (input.value as u8 in Wrapping) + 48;",
        ),
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Ascii requires ascii_only(self);
            data Input {{ value: u64 [0..=9]; }}
            machine corrupt(value: &mut u64) {{ value = 400; }}
            machine write(output: &mut [u8; 4], {input_decl}, position: u64 [0..=3])
            requires output in Ascii
            ensures output in Ascii {{ {body} }}
            "#
        );
        let diagnostics =
            lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
                .expect_err("borrowed storage must not retain its declared range");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{body}: {diagnostics:#?}"
        );
    }
}

#[test]
fn effectful_nested_call_arguments_keep_the_return_bounds_live() {
    // A selected scalar operand carries no hidden write footprint: a Unit
    // call nested inside the converting callee still lets the returned byte
    // prove the output predicate. An operand without a retained selected
    // form stays opaque and rejects.
    for (definitions, callee_body, succeeds) in [
        (
            "machine observe(value: u64) {}",
            "observe(value + 1); ((value % 10 + 48) as u8 in Wrapping) as u8",
            true,
        ),
        (
            "machine observe(value: u64) {}",
            "observe(value % 10); ((value % 10 + 48) as u8 in Wrapping) as u8",
            true,
        ),
        (
            "machine poke(cell: &mut u64) { cell = 255; }",
            "let mut scratch: u64 = 0; poke(&mut scratch); ((value % 10 + 48) as u8 in Wrapping) as u8",
            false,
        ),
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Ascii requires ascii_only(self);
            {definitions}
            machine digit(value: u64) -> u8 {{ {callee_body} }}
            machine write(output: &mut [u8; 4], position: u64 [0..=3], unknown: u64)
            requires output in Ascii
            ensures output in Ascii {{ output[position] = digit(unknown); }}
            "#
        );
        check(&source, succeeds);
    }
}

#[test]
fn mutable_formal_call_results_read_the_lent_places_incoming_value() {
    // A `mut` formal's incoming value is the caller's lent storage read at
    // the call point: a live local snapshot wins, storage behind an exclusive
    // borrow still supplies the formal's declared carrier, and nothing is
    // guessed from the callee's own later writes alone.
    for (body, succeeds) in [
        (
            "let mut scratch: u64 = 0; output[position] = digit(&mut scratch, unknown);",
            true,
        ),
        ("output[position] = digit(&mut pad.cell, unknown);", true),
        (
            "let mut scratch: u64 = 65; output[position] = widen(&mut scratch);",
            true,
        ),
        (
            // The lent snapshot 300 wraps concretely to 44, which is Ascii.
            "let mut scratch: u64 = 300; output[position] = widen(&mut scratch);",
            true,
        ),
        ("output[position] = widen(&mut pad.cell);", false),
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Ascii requires ascii_only(self);
            data Pad {{ cell: u64; }}
            machine digit(mut scratch: u64, value: u64) -> u8 {{ scratch = value; ((scratch % 10 + 48) as u8 in Wrapping) as u8 }}
            machine widen(mut scratch: u64) -> u8 {{ (scratch as u8 in Wrapping) as u8 }}
            machine write(output: &mut [u8; 4], pad: &mut Pad, position: u64 [0..=3], unknown: u64)
            requires output in Ascii
            ensures output in Ascii {{ {body} }}
            "#
        );
        check(&source, succeeds);
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
