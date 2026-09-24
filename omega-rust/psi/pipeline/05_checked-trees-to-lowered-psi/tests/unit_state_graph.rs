use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    TerminalEffect, TerminalExecutionResult, interpret_terminal_artifact_measured,
};

#[path = "unit_state_graph/cases.rs"]
mod cases;

#[path = "unit_state_graph/bindings.rs"]
mod bindings;

#[path = "unit_state_graph/scalars.rs"]
mod scalars;

#[path = "unit_state_graph/tails.rs"]
mod tails;

#[path = "unit_state_graph/reachable_states.rs"]
mod reachable_states;

#[path = "unit_state_graph/ranking.rs"]
mod ranking;

#[path = "unit_state_graph/provider_attachments.rs"]
mod provider_attachments;

#[path = "unit_state_graph/discarded_results.rs"]
mod discarded_results;

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

#[test]
fn free_unit_graph_preserves_view_scalar_edges_effect_order_and_continuation() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("free multistate helper belongs to the ordinary call closure");
    let result = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
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
