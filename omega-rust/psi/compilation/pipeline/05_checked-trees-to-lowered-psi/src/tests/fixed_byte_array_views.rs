//! Source-produced fixed-array loans retain initialized backing and exact extent.
use super::byte_sequence_write;
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
    let checked =
        crate::front_end::checked_program(&format!("{}\n{caller}", byte_sequence_write::PUT));
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
