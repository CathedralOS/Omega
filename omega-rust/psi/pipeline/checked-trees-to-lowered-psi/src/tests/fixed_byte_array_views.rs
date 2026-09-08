//! Source-produced fixed-array loans retain initialized backing and exact extent.

use super::*;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionStatus, TerminalStructuralByteArrayValue,
    TerminalStructuralValue,
};

fn array_fixture(length: usize, field: bool) -> terminal_codec::CanonicalTerminalArtifact {
    let caller = if field {
        format!(
            "data Record {{ out: [u8; {length}]; other: [u8; {length}]; }}\n\
             machine Record::run(&mut self) {{ put(&mut self.out, 65); put(&mut self.out, 0); }}"
        )
    } else {
        format!("machine run(out: &mut [u8; {length}]) {{ put(out, 65); put(out, 0); }}")
    };
    let checked = checked_source(&format!("{}\n{caller}", byte_sequence_write::PUT));
    produce_terminal_artifact(&checked, if field { "Record::run" } else { "run" })
        .expect("source-produced raw fixed array lends a mutable view")
}

fn entry_argument(artifact: &terminal_codec::CanonicalTerminalArtifact) -> TerminalStructuralValue {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    TerminalStructuralValue {
        opaque_identity: 73,
        structural_type: entry.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    }
}

#[test]
fn fixed_byte_array_views_write_original_storage_across_calls_and_suspension() {
    for initial in [vec![0x11], vec![0x11, 0x80, 0xff]] {
        for field in [false, true] {
            let artifact = array_fixture(initial.len(), field);
            let path = if field {
                vec![StructuralPathSegment::Field("out".into())]
            } else {
                Vec::new()
            };
            let sibling_path = vec![StructuralPathSegment::Field("other".into())];
            let mut arrays = vec![TerminalStructuralByteArrayValue {
                argument_index: 0,
                path: path.clone(),
                bytes: initial.clone(),
            }];
            if field {
                arrays.push(TerminalStructuralByteArrayValue {
                    argument_index: 0,
                    path: sibling_path.clone(),
                    bytes: vec![0x42; initial.len()],
                });
            }
            let mut execution =
                TerminalExecution::start_artifact_with_structural_arguments_and_byte_arrays(
                    artifact.semantic_bytes(),
                    artifact.proof_bytes(),
                    &proof_admission::AdmissionProfile::default(),
                    &[],
                    &[entry_argument(&artifact)],
                    &arrays,
                )
                .unwrap();
            let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
            let mut saw_first_write = initial.is_empty();
            let mut completed = false;
            for _ in 0..100 {
                let status = execution.resume(&mut fuel).unwrap();
                let observed = execution.structural_byte_array(73, &path).unwrap().to_vec();
                assert_eq!(observed.len(), initial.len(), "array extent cannot change");
                if !initial.is_empty() {
                    assert_eq!(&observed[1..], &initial[1..], "unwritten raw bytes survive");
                    saw_first_write |= observed[0] == 65;
                }
                if field {
                    assert_eq!(
                        execution.structural_byte_array(73, &sibling_path),
                        Some(vec![0x42; initial.len()].as_slice())
                    );
                }
                match status {
                    TerminalExecutionStatus::SponsorExhausted(_) => {
                        assert!(matches!(
                            execution.resume(&mut fuel).unwrap(),
                            TerminalExecutionStatus::SponsorExhausted(_)
                        ));
                        assert_eq!(
                            execution.structural_byte_array(73, &path),
                            Some(observed.as_slice()),
                            "exhausted fuel cannot repeat a write"
                        );
                        fuel.replenish(1).unwrap();
                    }
                    TerminalExecutionStatus::Complete(_) => {
                        let mut expected = initial.clone();
                        if let Some(first) = expected.first_mut() {
                            *first = 0;
                        }
                        assert_eq!(observed, expected);
                        assert!(execution.effects().is_empty());
                        completed = true;
                        break;
                    }
                    TerminalExecutionStatus::Crashed(crash) => {
                        panic!("unexpected crash: {crash:?}")
                    }
                }
            }
            assert!(
                completed && saw_first_write,
                "both guarded calls must execute on original storage"
            );
        }
    }
}

#[test]
fn fixed_byte_array_views_reject_mistyped_or_duplicate_initial_storage() {
    let artifact = array_fixture(3, false);
    let argument = entry_argument(&artifact);
    let valid = TerminalStructuralByteArrayValue {
        argument_index: 0,
        path: Vec::new(),
        bytes: vec![1, 2, 3],
    };
    let mut bad_path = valid.clone();
    bad_path
        .path
        .push(StructuralPathSegment::Field("invented".into()));
    let mut bad_argument = valid.clone();
    bad_argument.argument_index = 1;
    for arrays in [
        vec![TerminalStructuralByteArrayValue {
            bytes: vec![1, 2],
            ..valid.clone()
        }],
        vec![TerminalStructuralByteArrayValue {
            bytes: vec![1, 2, 3, 4],
            ..valid.clone()
        }],
        vec![valid.clone(), valid.clone()],
        vec![bad_path],
        vec![bad_argument],
    ] {
        assert!(
            TerminalExecution::start_artifact_with_structural_arguments_and_byte_arrays(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[],
                std::slice::from_ref(&argument),
                &arrays,
            )
            .is_err()
        );
    }
    let start = TerminalExecution::start_artifact_with_structural_arguments_and_byte_arrays(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        &[argument],
        &[],
    );
    if let Ok(mut execution) = start {
        let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(100);
        assert!(
            execution.resume(&mut fuel).is_err(),
            "an opaque identity does not initialize its array"
        );
    }
}

#[test]
fn fixed_byte_array_views_replay_authored_field_and_access() {
    let checked = checked_source(&format!(
        "{}\n data Record {{ out: [u8;3]; other: [u8;3]; }}\n\
         machine Record::run(&mut self) {{ put(&mut self.out,65); }}",
        byte_sequence_write::PUT
    ));
    let _artifact = produce_terminal_artifact(&checked, "Record::run").unwrap();
    for change_access in [false, true] {
        let mut changed = checked.clone();
        let call = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|plan| &mut plan.operations)
            .find(|operation| matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. }))
            .unwrap();
        let CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        } = call
        else {
            panic!("call")
        };
        if change_access {
            structural_arguments[0].access = checked_trees::CheckedStructuralAccess::SharedBorrow;
        } else {
            structural_arguments[0].path =
                vec![CheckedUnitStructuralPathSegment::Field("other".into())];
        }
        assert!(
            lower_machine(&changed, "Record::run").is_err(),
            "same-typed field/access substitution rejects"
        );
    }
}
