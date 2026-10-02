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
                    .unwrap_or_else(|error| {
                        panic!("window {start}..{end}, field={field}: {error:?}")
                    }),
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
fn bounded_byte_parameter_range_executes_the_authored_caller() {
    let checked = crate::front_end::checked_program(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/omega/pass/slices/bounded_byte_parameter_range_exit/main.omg"
    )));
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("the caller and bounded helper publish")
    .into_artifact();
    let mut changed_capacity = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let bounded = changed_capacity
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. },
            ) => Some(capacity),
            _ => None,
        })
        .expect("the actual helper retains its bounded formal");
    assert_eq!(*bounded, 3);
    *bounded = 4;
    assert!(
        terminal_verifier::validate_module(&changed_capacity).is_err(),
        "the caller's three-byte field cannot satisfy a four-byte reference"
    );
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .unwrap();
    assert_eq!(
        execution
            .resume(
                &mut terminal_fuel::TerminalFuelMeter::with_allowance(100_000),
                &mut AcceptTerminalEffects,
            )
            .unwrap_or_else(|error| panic!("authored bounded caller execution: {error:?}")),
        TerminalExecutionStatus::Complete(terminal_interpreter::TerminalExecutionResult::Unit,),
        "the authored guard observes [17, 0, 255] and marker 42 after both calls"
    );
}

#[test]
fn bounded_byte_range_establishment_rejects_access_and_descriptor_drift() {
    let artifact = array_fixture(3, false, Some((1, 3)));
    let original = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    for mutation in ["source access", "element", "element descriptor"] {
        let mut module = original.clone();
        let operation = module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::EstablishMutableElementView { .. }
                )
            })
            .expect("bounded backing establishes an ordinary descriptor");
        let result_type = operation.result.structural().unwrap().structural_type;
        let terminal_psi::OperationKind::EstablishMutableElementView {
            source, element, ..
        } = &mut operation.kind
        else {
            unreachable!()
        };
        match mutation {
            "source access" => source.access = terminal_psi::StructuralAccess::SharedBorrow,
            "element" => *element = result_type,
            "element descriptor" => {
                let declaration = module
                    .structural_types
                    .iter_mut()
                    .find(|declaration| declaration.id == result_type)
                    .unwrap();
                assert!(declaration.shape.is_borrowed_byte_view());
                declaration.shape =
                    terminal_psi::StructuralTypeShape::ElementView { element: *element };
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::validate_module(&module).is_err(),
            "bounded byte establishment accepted {mutation}"
        );
    }
}
