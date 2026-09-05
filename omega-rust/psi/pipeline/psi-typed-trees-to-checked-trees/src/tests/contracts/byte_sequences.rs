use super::*;

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
    lower_typed_trees(parse_typed_trees(&source)).expect("nested concatenation establishes output");
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
    lower_typed_trees(parse_typed_trees(&source)).expect("copied bytes are independent of input");
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
        let diagnostics = lower_typed_trees(parse_typed_trees(&source)).expect_err(mutation);
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
    lower_typed_trees(parse_typed_trees(&source)).expect("in-place append reads the old value");
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
        let result = lower_typed_trees(parse_typed_trees(&source));
        assert_eq!(
            result.is_ok(),
            succeeds,
            "{predicate}/{literal}: {result:#?}"
        );
    }
}
