//! Source-produced fixed-array loans retain initialized backing and exact extent.
use super::{byte_sequence_write, checked_source, lower_machine};
use crate::TerminalMachineSelection;
use checked_trees::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment};
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionStatus, TerminalStructuralByteArrayValue,
    TerminalStructuralValue,
};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::StructuralPathSegment;

fn array_fixture(
    length: usize,
    field: bool,
    window: Option<(usize, usize)>,
) -> terminal_codec::CanonicalTerminalArtifact {
    let argument = match (field, window) {
        (true, None) => "&mut self.out".to_owned(),
        (false, None) => "out".to_owned(),
        (true, Some((start, end))) => format!("&mut self.out[{start}..{end}]"),
        (false, Some((start, end))) => format!("&mut out[{start}..{end}]"),
    };
    let caller = if field {
        format!(
            "data Record {{ out: [u8; {length}]; other: [u8; {length}]; }}\n\
             machine Record::run(&mut self) {{ put({argument}, 65); put({argument}, 0); }}"
        )
    } else {
        format!(
            "machine run(out: &mut [u8; {length}]) {{ put({argument}, 65); put({argument}, 0); }}"
        )
    };
    let checked = checked_source(&format!("{}\n{caller}", byte_sequence_write::PUT));
    terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name(if field {
            "Record::run"
        } else {
            "run"
        }),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("source-produced raw fixed array lends a mutable view")
    .into_artifact()
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
            let artifact = array_fixture(initial.len(), field, None);
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
            let mut execution = TerminalExecution::start_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[],
                TerminalStructuralInputs {
                    arguments: &[entry_argument(&artifact)],
                    byte_arrays: &arrays,
                    ..Default::default()
                },
            )
            .unwrap();
            let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
            let mut saw_first_write = initial.is_empty();
            let mut completed = false;
            for _ in 0..100 {
                let status = execution
                    .resume(&mut fuel, &mut AcceptTerminalEffects)
                    .unwrap();
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
                            execution
                                .resume(&mut fuel, &mut AcceptTerminalEffects)
                                .unwrap(),
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
    let artifact = array_fixture(3, false, None);
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
            TerminalExecution::start_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[],
                TerminalStructuralInputs {
                    arguments: std::slice::from_ref(&argument),
                    byte_arrays: &arrays,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
    let start = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            ..Default::default()
        },
    );
    if let Ok(mut execution) = start {
        let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(100);
        assert!(
            execution
                .resume(&mut fuel, &mut AcceptTerminalEffects)
                .is_err(),
            "an opaque identity does not initialize its array"
        );
    }
}

#[test]
fn fixed_byte_windows_publish_and_execute_repeated_call_loans() {
    for (start, end) in [(0, 0), (1, 3), (3, 3), (0, 3)] {
        for field in [false, true] {
            let artifact = array_fixture(3, field, Some((start, end)));
            let path = if field {
                vec![StructuralPathSegment::Field("out".into())]
            } else {
                Vec::new()
            };
            let sibling_path = vec![StructuralPathSegment::Field("other".into())];
            let mut arrays = vec![TerminalStructuralByteArrayValue {
                argument_index: 0,
                path: path.clone(),
                bytes: vec![17, 128, 255],
            }];
            if field {
                arrays.push(TerminalStructuralByteArrayValue {
                    argument_index: 0,
                    path: sibling_path.clone(),
                    bytes: vec![42; 3],
                });
            }
            let mut execution = TerminalExecution::start_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[],
                TerminalStructuralInputs {
                    arguments: &[entry_argument(&artifact)],
                    byte_arrays: &arrays,
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(matches!(
                execution
                    .resume(
                        &mut terminal_fuel::TerminalFuelMeter::unbounded(),
                        &mut AcceptTerminalEffects
                    )
                    .unwrap(),
                TerminalExecutionStatus::Complete(_)
            ));
            let mut expected = [17, 128, 255];
            if start < end {
                expected[start] = 0;
            }
            assert_eq!(
                execution.structural_byte_array(73, &path).unwrap(),
                expected
            );
            if field {
                assert_eq!(
                    execution.structural_byte_array(73, &sibling_path).unwrap(),
                    &[42; 3]
                );
            }
        }
    }
}

#[test]
fn fixed_byte_windows_replay_authored_endpoints() {
    for invocation in ["put(&mut self.out[1..3],65)", "observe(&self.out[1..3])"] {
        let checked = checked_source(&format!(
            "{}\n machine observe(bytes: &[u8]) {{}}\n data Record {{ out: [u8;3]; }}\n\
         machine Record::run(&mut self) {{ {invocation}; }}",
            byte_sequence_write::PUT
        ));
        let _artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name("Record::run"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .unwrap()
        .into_artifact();
        // Each substitute is itself a valid array window. Bounds validation alone
        // cannot establish that it is the window the caller actually authored.
        for (start, end) in [(0, 2), (1, 2), (3, 3)] {
            let mut changed = checked.clone();
            let call = changed
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .flat_map(|plan| &mut plan.operations)
                .find(|operation| {
                    matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
                })
                .unwrap();
            let CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            } = call
            else {
                panic!("call");
            };
            *structural_arguments[0].path.last_mut().unwrap() =
                CheckedUnitStructuralPathSegment::FixedByteRange { start, end };
            assert!(
                lower_machine(&changed, TerminalMachineSelection::Name("Record::run")).is_err(),
                "a valid but unauthored byte window must reject"
            );
        }
    }
}

#[test]
fn fixed_byte_array_views_replay_authored_field_and_access() {
    let checked = checked_source(&format!(
        "{}\n data Record {{ out: [u8;3]; other: [u8;3]; }}\n\
         machine Record::run(&mut self) {{ put(&mut self.out,65); }}",
        byte_sequence_write::PUT
    ));
    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Record::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
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
            lower_machine(&changed, TerminalMachineSelection::Name("Record::run")).is_err(),
            "same-typed field/access substitution rejects"
        );
    }
}
