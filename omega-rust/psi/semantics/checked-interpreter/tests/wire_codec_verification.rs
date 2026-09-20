//! GENERATED-CODEC-INDEPENDENT-VERIFICATION: a synthesized wire codec's
//! report row may claim `Derived` trust only after its generated bodies are
//! checked against the public compact_binary requirement. These tests drive
//! `checked_interpreter::verify_wire_schema_codec` over schemas covering
//! every field kind the stage-2 realization serves, and pin that an
//! encode-only field kind keeps the row's strict-decode coverage open.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("wire tokens");
    let syntax = parse_syntax_trees(&tokens).expect("wire syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("wire symbols");
    lower_symbol_resolved_trees(&resolved).expect("wire types")
}

fn verify(schema_source: &str, schema_name: &str) -> checked_interpreter::WireCodecVerification {
    let typed = typed(&format!(
        "{schema_source}
         machine main() -> i32 {{
             transition true {{ true -> 7 false -> 0 }}
         }}"
    ));
    let schema = typed
        .wire_schemas()
        .iter()
        .find(|schema| schema.name.as_str() == schema_name)
        .unwrap_or_else(|| panic!("no wire schema `{schema_name}`"));
    checked_interpreter::verify_wire_schema_codec(&typed, schema)
        .unwrap_or_else(|divergence| panic!("{schema_name}: {divergence}"))
}

#[test]
fn scalar_schema_is_fully_derived() {
    let verification = verify(
        "data Packet {
             #0 tag: u32;
             #1 depth: i32;
         }",
        "Packet",
    );
    assert!(verification.gaps.is_empty(), "{:?}", verification.gaps);
    assert_eq!(verification.checks.len(), 3, "{:?}", verification.checks);
    assert!(verification.checks[0].contains("canonical emission"));
    assert!(verification.checks[1].contains("round-trips"));
    assert!(verification.checks[2].contains("rejects"));
}

#[test]
fn every_stage_two_field_kind_round_trips() {
    let verification = verify(
        "data Header {
             #0 room: u32;
         }
         data Packet {
             #0 flag: bool;
             #1 header: Header;
             #2 bytes: &[u8];
             #3 pins: [i32; 4];
             #4 level: u32 [0..=9];
         }",
        "Packet",
    );
    assert!(verification.gaps.is_empty(), "{:?}", verification.gaps);
    // canonical emission, round-trip, rejection probes, and the body-length
    // overrun probe the nested/repeated fields enabled.
    assert_eq!(verification.checks.len(), 3, "{:?}", verification.checks);
}

#[test]
fn borrowed_scalar_slice_keeps_strict_decode_open() {
    // `&[i32]` has no decode-side field kind: the encode emission is still
    // independently verified, but the codec row must not claim Derived.
    let verification = verify(
        "data Telemetry {
             #0 readings: &[i32];
         }",
        "Telemetry",
    );
    assert!(
        verification.gaps.iter().any(|gap| gap.contains("readings")),
        "{:?}",
        verification.gaps
    );
    assert!(
        verification
            .checks
            .iter()
            .any(|check| check.contains("canonical emission"))
    );
}
