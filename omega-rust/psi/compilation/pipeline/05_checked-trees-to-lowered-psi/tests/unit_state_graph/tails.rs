use super::{
    AdmissionProfile, TerminalEffect, TerminalExecutionResult, encode_module, encode_proof_section,
    interpret_terminal_artifact_measured,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
const SOURCE: &str = r#"
    boundary trait Output { machine write(bytes: &[u8], marker: i32) reaches Output; }
    machine relay(bytes: &[u8]) reaches Output {
        transition bytes.len > 0 {
            true -> tail(bytes[1..], bytes[0] as i32)
            false -> empty()
        }
        state tail(bytes: &[u8], head: i32) {
            Output::write(bytes, head);
            transition bytes.len > 0 {
                true -> final_tail(bytes[1..], bytes[0] as i32)
                false -> empty()
            }
        }
        state final_tail(bytes: &[u8], head: i32) { Output::write(bytes, head); }
        state empty() { }
    }
    data Root {}
    machine Root::enter() reaches Output {
        relay("\x80AB");
        relay("");
        relay("Z");
        Output::write("last", 3i32);
    }
"#;

fn loop_source() -> String {
    SOURCE
        .replace(
            "true -> final_tail(bytes[1..], bytes[0] as i32)",
            "true -> tail(bytes[1..], bytes[0] as i32)",
        )
        .replace(
            "state final_tail(bytes: &[u8], head: i32) { Output::write(bytes, head); }",
            "",
        )
}

#[test]
fn unguarded_cyclic_byte_operations_reject_at_source_checking() {
    let source = loop_source().replace("transition bytes.len > 0 {", "transition true {");
    let errors =
        crate::front_end::checked_program_result(&source).expect_err("bounds remain required");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("cannot prove index"))
    );
}

#[test]
fn reentered_entry_binds_the_current_view_without_changing_invocation_parameters() {
    let source = r#"
        boundary trait Output { machine write(bytes: &[u8], marker: i32) reaches Output; }
        machine relay(bytes: &[u8]) reaches Output {
            transition bytes.len > 0 {
                true -> emit(bytes[0] as i32, bytes[1..])
                false -> done()
            }
            state emit(head: i32, bytes: &[u8]) {
                Output::write(bytes, head);
                transition { _ -> relay(bytes) }
            }
            state done() { }
        }
        data Root {}
        machine Root::enter() reaches Output {
            relay("\x80AB"); relay(""); relay("Z"); Output::write("last", 3i32);
        }
    "#;
    assert_eq!(
        effects(source),
        vec![
            (b"AB".to_vec(), 128),
            (b"B".to_vec(), 65),
            (Vec::new(), 66),
            (Vec::new(), 90),
            (b"last".to_vec(), 3),
        ]
    );
}

fn effects(source: &str) -> Vec<(Vec<u8>, i128)> {
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("selected tail descriptors reach later states");
    let execution = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("tail bounds and descriptor custody independently verify and execute");
    assert_eq!(execution.value(), TerminalExecutionResult::Unit);
    execution
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
            let [
                terminal_interpreter::TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Signed(marker),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("one signed marker")
            };
            (byte_sequence_arguments[0].clone().unwrap(), *marker)
        })
        .collect()
}

#[test]
fn endpoint_bindings_cannot_move_between_state_edges() {
    let checked = crate::front_end::checked_program(SOURCE);
    let role = checked_trees::CheckedScalarExpressionRole::SubsliceStart {
        site: checked_trees::CheckedSubsliceSite::TransitionArgument {
            argument_ordinal: 0,
        },
    };
    let handles = checked
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .iter()
        .filter(|(_, binding)| binding.role == role)
        .map(|(handle, binding)| (handle, binding.expression))
        .collect::<Vec<_>>();
    assert_eq!(handles.len(), 2);
    let mut changed = checked.clone();
    changed
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .get_mut(handles[0].0)
        .expression = handles[1].1;
    checked_trees_to_lowered_psi::lower_machine(
        &changed,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect_err("equal endpoint values from different source edges are not interchangeable");
}

#[test]
fn reversed_boolean_label_pair_tails_lower_and_execute_the_inverse_arm() {
    let source = r#"
        boundary trait Output { machine write(bytes: &[u8], marker: i32) reaches Output; }
        machine dispatch(flag: bool, bytes: &[u8]) reaches Output {
            transition {
                flag == false -> low(bytes)
                flag == true -> high(bytes)
            }
            state low(bytes: &[u8]) { Output::write(bytes, 1i32); }
            state high(bytes: &[u8]) { Output::write(bytes, 2i32); }
        }
        data Root {}
        machine Root::enter() reaches Output {
            dispatch(true, "hit");
            dispatch(false, "miss");
        }
    "#;
    assert_eq!(
        effects(source),
        vec![(b"hit".to_vec(), 2), (b"miss".to_vec(), 1)]
    );
}
