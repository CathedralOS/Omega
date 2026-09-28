use super::{
    AdmissionProfile, TerminalEffect, TerminalExecutionResult, encode_module, encode_proof_section,
    interpret_terminal_artifact_measured,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_psi::StructuralPathSegment;

const SOURCE: &str = r#"
    boundary trait Output { machine write(bytes: &[u8], marker: u8) reaches Output; }
    data Reply { case Done; }
    machine probe(bytes: &[u8], marker: u8) -> Reply reaches Output {
        Output::write(bytes, marker);
        transition { _ -> finish() }
        state finish() -> Reply { Reply::Done }
    }
    data Root {}
    machine Root::enter() reaches Output {
        _ = probe("first", 1u8);
        Output::write("middle", 2u8);
        _ = probe("last", 3u8);
    }
"#;

#[test]
fn discarded_structural_calls_preserve_results_and_interleaved_effects() {
    for source in [
        SOURCE.to_owned(),
        SOURCE.replace(
            "Output::write(\"middle\", 2u8);",
            "let marker: u8 = 2u8; Output::write(\"middle\", marker);",
        ),
        SOURCE.replace(
            "_ = probe(\"first\", 1u8);",
            "let ignored: Reply = probe(\"first\", 1u8);",
        ),
    ] {
        assert_eq!(
            effects(&source),
            [b"first".to_vec(), b"middle".to_vec(), b"last".to_vec()]
        );
    }
}

fn effects(source: &str) -> Vec<Vec<u8>> {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(source),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("discard is a result disposition, not a Unit signature");
    let execution = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("independently verified discarded results execute");
    assert_eq!(execution.value(), TerminalExecutionResult::Unit);
    execution
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall {
                byte_sequence_arguments,
                ..
            } = effect
            else {
                panic!("output effect");
            };
            byte_sequence_arguments[0].clone().unwrap()
        })
        .collect()
}

const BUFFER_SOURCE: &str = r#"
        data Reply { case Done; }
        machine fill(out: &mut [u8]) -> Reply {
            transition out.len > 0 { true -> store(out) false -> done() }
            state store(out: &mut [u8]) -> Reply { out[0] = 42; Reply::Done }
            state done() -> Reply { Reply::Done }
        }
        data Root { buffer: [u8; 1]; }
        machine Root::enter(&mut self) { _ = fill(&mut self.buffer); }
    "#;

#[test]
fn projected_result_call_rejects_forged_paths_and_access() {
    let baseline = checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(BUFFER_SOURCE),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .unwrap()
    .semantic_module;
    terminal_verifier::validate_module(&baseline).unwrap();
    for mutation in 0..5 {
        let mut module = baseline.clone();
        let entry = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let argument = entry
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| {
                if let terminal_psi::OperationKind::CallStructuralWithScalarArguments {
                    structural_arguments,
                    ..
                } = &mut operation.kind
                {
                    Some(&mut structural_arguments[0])
                } else {
                    None
                }
            })
            .unwrap();
        match mutation {
            0 => argument.path = vec![StructuralPathSegment::Field("missing".into())],
            1 => argument.path.push(StructuralPathSegment::FixedIndex(0)),
            2 => argument.access = terminal_psi::StructuralAccess::SharedBorrow,
            3 => argument.access = terminal_psi::StructuralAccess::Owned,
            4 => {
                entry.structural_parameters[0].access = terminal_psi::StructuralAccess::SharedBorrow
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::validate_module(&module).is_err(),
            "mutation {mutation}"
        );
    }
}
