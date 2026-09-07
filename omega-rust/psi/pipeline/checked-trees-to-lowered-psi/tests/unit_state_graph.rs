use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalEffect, TerminalExecutionResult, interpret_terminal_artifact_measured,
};
use tokens_to_syntax_trees::parse_syntax_trees;

#[path = "unit_state_graph/cases.rs"]
mod cases;

#[path = "unit_state_graph/scalars.rs"]
mod scalars;

#[path = "unit_state_graph/tails.rs"]
mod tails;

const SOURCE: &str = r#"
    boundary trait Output { machine write(bytes: &[u8], marker: u8) reaches Output; }
    machine relay(bytes: &[u8], selected: bool, marker: u8) reaches Output {
        Output::write(bytes, marker);
        transition selected {
            true -> first(bytes, marker)
            false -> second(bytes, marker)
        }
        state first(bytes: &[u8], marker: u8) {
            Output::write(bytes, 1u8);
            transition { _ -> finish(bytes, marker) }
        }
        state second(bytes: &[u8], marker: u8) {
            Output::write(bytes, 2u8);
            transition { _ -> finish(bytes, marker) }
        }
        state finish(bytes: &[u8], marker: u8) {
            Output::write(bytes, marker);
        }
    }
    data Root {}
    machine Root::enter() reaches Output {
        relay("\x80A", true, 7u8);
        relay("", false, 9u8);
        Output::write("last", 3u8);
    }
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check")
}

#[test]
fn free_unit_graph_preserves_view_scalar_edges_effect_order_and_continuation() {
    let checked = checked(SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("free multistate helper belongs to the ordinary call closure");
    let result = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
    )
    .expect("source-independent graph execution");
    assert_eq!(result.value(), TerminalExecutionResult::Unit);
    let output = result
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall {
                byte_sequence_arguments,
                arguments,
                ..
            } = effect
            else {
                panic!("expected output boundary");
            };
            (
                byte_sequence_arguments[0].clone().expect("byte view"),
                arguments.clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(output.len(), 7);
    assert_eq!(
        output
            .iter()
            .map(|(bytes, _)| bytes.as_slice())
            .collect::<Vec<_>>(),
        [
            b"\x80A".as_slice(),
            b"\x80A",
            b"\x80A",
            b"",
            b"",
            b"",
            b"last"
        ]
    );
    let markers = output
        .iter()
        .map(|(_, arguments)| {
            assert_eq!(arguments.len(), 1);
            arguments[0]
        })
        .collect::<Vec<_>>();
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
    use terminal_interpreter::TerminalScalarValue;
    assert_eq!(
        markers,
        [7, 1, 7, 9, 2, 9, 3].map(|value| TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            value: IntegerValue::Unsigned(value),
        })
    );
}
