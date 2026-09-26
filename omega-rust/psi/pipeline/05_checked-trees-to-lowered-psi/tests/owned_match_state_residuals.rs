//! Owned match selections inside an authored state carry their source roster
//! through the transition join: the selected result and interleaved live
//! owners bind successor parameters positionally, while the displaced source
//! dies on the selected edge.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    TerminalEffect, TerminalExecutionResult, TerminalScalarValue,
    interpret_terminal_artifact_measured,
};

const SOURCE: &str =
    include_str!("../../../../../tests/omega/pass/expressions/owned_match_authored_state/main.omg");

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
        value: IntegerValue::Unsigned(value),
    }
}

fn lowered() -> (checked_trees::CheckedTrees, lowered_psi::LoweredPsi) {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect("selection residuals transport across the state edge");
    (checked, lowered)
}

fn recorded_source(source: &str, selected: bool) -> Vec<u128> {
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect("selection residuals transport across the state edge");
    let semantic_bytes = encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    let module = terminal_codec::decode_module(&semantic_bytes).unwrap();
    let proof = terminal_codec::decode_proof_bundle(&proof_bytes).unwrap();
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("independent verification");
    let execution = interpret_terminal_artifact_measured(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(selected), unsigned(41)],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("execution");
    assert_eq!(execution.value(), TerminalExecutionResult::Unit);
    execution
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("expected sink boundary");
            };
            let [
                TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(value),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("one unsigned record");
            };
            *value
        })
        .collect()
}

fn recorded(selected: bool) -> Vec<u128> {
    recorded_source(SOURCE, selected)
}

#[test]
fn owned_match_state_residuals_replay_both_arms() {
    // The selected arm decides which sum constructor reaches `kept`: the bonus
    // records 1 or 2, while the transported interleaved owner always records 41.
    assert_eq!(recorded(true), [41, 1]);
    assert_eq!(recorded(false), [41, 2]);
}

#[test]
fn owned_match_state_residual_selected_result_dies_on_edge() {
    // The same selection with the result retired on the selected edge instead
    // of transferred: each successor disposes the result parameter and the
    // residual source parameter at their own roster positions.
    let source = SOURCE
        .replace("kept(result, marker)", "kept(marker, observed)")
        .replace(
            "state kept(result: Choice, marker: Marker)",
            "state kept(marker: Marker, observed: bool)",
        )
        .replace("match result in Choice::Some", "match observed");
    assert_eq!(recorded_source(&source, true), [41, 1]);
    assert_eq!(recorded_source(&source, false), [41, 2]);
}

#[test]
fn owned_match_state_residuals_reject_mutated_edge_cleanup() {
    let (_, lowered) = lowered();
    let module = lowered.semantic_module;
    for mutation in [
        Mutation::DropResidual,
        Mutation::DuplicateResidual,
        Mutation::SwapArguments,
    ] {
        let mut changed = module.clone();
        let machine = changed
            .machines
            .iter_mut()
            .find(|machine| machine.id == changed.entry)
            .unwrap();
        let mut mutated = 0;
        for block in &mut machine.blocks {
            let terminal_psi::Terminator::Conditional {
                when_true,
                when_false,
                ..
            } = &mut block.terminator
            else {
                continue;
            };
            for edge in [when_true, when_false] {
                if edge.trivial_affine_discards.is_empty() {
                    continue;
                }
                match mutation {
                    Mutation::DropResidual => edge.trivial_affine_discards.clear(),
                    Mutation::DuplicateResidual => {
                        let place = edge.trivial_affine_discards[0];
                        edge.trivial_affine_discards.push(place);
                    }
                    Mutation::SwapArguments => {
                        edge.structural_arguments.swap(0, 1);
                    }
                }
                mutated += 1;
            }
        }
        assert_eq!(mutated, 2, "both successors drop the residual source");
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &lowered.proof_bundle,
                &AdmissionProfile::default()
            )
            .is_err(),
            "mutation {mutation:?} must reject"
        );
    }
}

#[derive(Debug, Clone, Copy)]
enum Mutation {
    DropResidual,
    DuplicateResidual,
    SwapArguments,
}
