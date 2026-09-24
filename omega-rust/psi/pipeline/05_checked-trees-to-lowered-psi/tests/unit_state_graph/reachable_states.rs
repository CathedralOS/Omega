use super::{
    AdmissionProfile, TerminalEffect, TerminalExecutionResult, encode_module, encode_proof_section,
    interpret_terminal_artifact_measured,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::AcceptTerminalEffects;

const SOURCE: &str = r#"
    boundary trait Output { machine write(bytes: &[u8], marker: u8) reaches Output; }
    machine relay(bytes: &[u8], marker: u8) reaches Output {
        Output::write(bytes, marker);
        transition { _ -> done(bytes) }
        state done(bytes: &[u8]) {
            Output::write(bytes, 1u8);
        }
        state dead(bytes: &[u8]) {
            Output::write(bytes, 9u8);
            transition { _ -> done(bytes) }
        }
    }
    data Root {}
    machine Root::enter() reaches Output {
        relay("\x80A", 7u8);
    }
"#;

fn markers() -> Vec<u8> {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("unreachable states prune out of the emitted unit graph");
    let execution = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
        terminal_interpreter::TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("verified graph executes");
    assert_eq!(execution.value(), TerminalExecutionResult::Unit);
    execution
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("expected output boundary");
            };
            let [
                terminal_interpreter::TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Unsigned(marker),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("one marker argument");
            };
            *marker as u8
        })
        .collect()
}

#[test]
fn unreachable_state_prunes_from_the_emitted_unit_graph() {
    assert_eq!(markers(), vec![7, 1]);
}

const VETO_SOURCE: &str = r#"
    boundary trait Output { machine write(bytes: &[u8], marker: u8) reaches Output; }
    data Cell { filled: u8 }
    machine relay(bytes: &[u8], marker: u8) reaches Output {
        Output::write(bytes, marker);
        transition { _ -> done(bytes) }
        state done(bytes: &[u8]) {
            Output::write(bytes, 1u8);
        }
        state dead(scratch: &write Cell) {
            Output::write("", 9u8);
        }
    }
    data Root {}
    machine Root::enter() reaches Output {
        relay("\x80A", 7u8);
    }
"#;

fn markers_for(source: &str) -> Vec<u8> {
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("unreachable-state custody rows must not veto the route");
    let execution = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
        terminal_interpreter::TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("verified graph executes");
    execution
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("expected output boundary");
            };
            let [
                terminal_interpreter::TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Unsigned(marker),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("one marker argument");
            };
            *marker as u8
        })
        .collect()
}

#[test]
fn unreachable_state_write_borrow_custody_cannot_veto_the_route() {
    assert_eq!(markers_for(VETO_SOURCE), vec![7, 1]);
}

#[test]
fn reachable_state_write_borrow_custody_still_vetoes_the_route() {
    let live_veto: &str = r#"
    boundary trait Output { machine write(bytes: &[u8], marker: u8) reaches Output; }
    data Cell { filled: u8 }
    machine relay(bytes: &[u8], marker: u8, cell: &write Cell) reaches Output {
        Output::write(bytes, marker);
        transition { _ -> done(bytes) }
        state done(bytes: &[u8]) {
            Output::write(bytes, 1u8);
        }
    }
"#;
    let checked = crate::front_end::checked_program(live_veto);
    let error = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("relay"),
    )
    .expect_err("reachable-state custody veto still declines the route");
    assert!(format!("{error:?}").contains("custody"), "{error:?}");
}
