//! A consuming move out of a mutable-borrowed root leaves a restoration
//! window the interpreter replays exactly once: the store relocates the
//! replacement subtree back under the caller's root, and the caller's view of
//! the field afterward answers the updated contents.

use super::{
    AcceptTerminalEffects, AdmissionProfile, BindingRelevance, Operation, OperationKind,
    OperationResult, ProofBundle, ScalarType, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalFuelMeter, TerminalMachineResult,
    TerminalModule, TerminalScalarValue, Terminator, ValueDeclaration, decode_module, edge_id,
    encode_module, encode_proof_section, operation_id, place_id, structural_field_id,
    structural_type_id, unit_module, value_id, verify_module,
};
use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralPlaceKind};
use terminal_interpreter::{TerminalStructuralScalarFieldValue, TerminalStructuralValue};
use terminal_psi::{RecordFieldInitializer, RecordFieldValue};

fn structural_type_declarations() -> Vec<StructuralTypeDeclaration> {
    let declaration = |id: u64, name: &str, field_type| StructuralFieldDeclaration {
        id: structural_field_id(id),
        identity: name.into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    vec![
        StructuralTypeDeclaration {
            id: structural_type_id(1),
            identity: "Cell".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![declaration(
                    1,
                    "flag",
                    StructuralFieldType::Scalar(ScalarType::Boolean),
                )],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(2),
            identity: "Envelope".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    declaration(
                        1,
                        "left",
                        StructuralFieldType::Structural(structural_type_id(1)),
                    ),
                    declaration(
                        2,
                        "right",
                        StructuralFieldType::Structural(structural_type_id(1)),
                    ),
                ],
            },
        },
    ]
}

/// One machine: move `left` out of the borrowed `Envelope` at `place 1`,
/// establish a fresh `Cell` with `flag = false`, reseat the window, then read
/// `left.flag` back through the borrowed root and return it.
fn window_module() -> TerminalModule {
    let mut module = unit_module();
    module.structural_types = structural_type_declarations();
    let machine = &mut module.machines[0];
    machine.structural_parameters = vec![StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: false,
        structural_type: structural_type_id(2),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: place_id(1),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(2),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(1),
                structural_type: structural_type_id(1),
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(3),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(3),
                structural_type: structural_type_id(1),
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(4),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(6),
                structural_type: structural_type_id(1),
            },
        },
    ];
    let structural_result = |place: u64| {
        OperationResult::Structural(StructuralOperationResult {
            place: place_id(place),
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        })
    };
    machine.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: operation_id(1),
            result: structural_result(2),
            kind: OperationKind::MoveStructuralField {
                source: place_id(1),
                path: Vec::new(),
                field: structural_field_id(1),
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(2),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(1),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanConstant { value: false },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(3),
            result: structural_result(3),
            kind: OperationKind::EstablishRecord {
                fields: vec![RecordFieldInitializer {
                    field: structural_field_id(1),
                    value: RecordFieldValue::Scalar {
                        value: value_id(1),
                        range_obligation: None,
                    },
                }],
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(4),
            result: OperationResult::Unit,
            kind: OperationKind::StoreStructuralField {
                destination: place_id(1),
                path: Vec::new(),
                field: structural_field_id(1),
                value: StructuralArgument {
                    place: place_id(3),
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(6),
            result: structural_result(4),
            kind: OperationKind::MoveStructuralField {
                source: place_id(1),
                path: Vec::new(),
                field: structural_field_id(2),
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(7),
            result: OperationResult::Unit,
            kind: OperationKind::StoreStructuralField {
                destination: place_id(1),
                path: Vec::new(),
                field: structural_field_id(2),
                value: StructuralArgument {
                    place: place_id(2),
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(5),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(2),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanStructuralField {
                source: place_id(1),
                path: vec![CanonicalStructuralPathSegment::Field(structural_field_id(
                    1,
                ))],
                field: structural_field_id(1),
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(8),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(4),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanStructuralField {
                source: place_id(1),
                path: vec![CanonicalStructuralPathSegment::Field(structural_field_id(
                    2,
                ))],
                field: structural_field_id(1),
            },
        },
    ];
    machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(4),
        cleanup_actions: Vec::new(),
    };
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(3),
        scalar_type: ScalarType::Boolean,
    });
    module
}

/// The entry arguments: one caller-owned `Envelope` whose `left` cell starts
/// with `flag = true` and whose `right` cell carries `flag = true` as well.
fn structural_inputs(
    left_flag: bool,
    right_flag: bool,
) -> (
    [TerminalStructuralValue; 1],
    [TerminalStructuralScalarFieldValue; 2],
) {
    (
        [TerminalStructuralValue {
            opaque_identity: 7,
            structural_type: structural_type_id(2),
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
        [
            TerminalStructuralScalarFieldValue {
                argument_index: 0,
                path: vec![StructuralPathSegment::Field("left".into())],
                field: structural_field_id(1),
                value: TerminalScalarValue::Boolean(left_flag),
            },
            TerminalStructuralScalarFieldValue {
                argument_index: 0,
                path: vec![StructuralPathSegment::Field("right".into())],
                field: structural_field_id(1),
                value: TerminalScalarValue::Boolean(right_flag),
            },
        ],
    )
}

#[test]
fn replacement_reseat_updates_the_caller_visible_subtree() {
    let module = window_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("move then repair verifies");
    let bytes = encode_module(&module).expect("window module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), module);
    let proof = encode_proof_section(&module, &ProofBundle::default()).unwrap();
    let (arguments, scalar_fields) = structural_inputs(true, true);
    // `right.flag` answers the subtree that was moved out of `left`: the
    // caller's view carries the swap, not the original contents.
    let expected = TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
        TerminalScalarValue::Boolean(true),
    ));
    let start = || {
        TerminalExecution::start_artifact(
            &bytes,
            &proof,
            &AdmissionProfile::default(),
            &[],
            terminal_interpreter::TerminalStructuralInputs {
                arguments: &arguments,
                scalar_fields: &scalar_fields,
                ..Default::default()
            },
        )
        .unwrap()
    };
    let mut execution = start();
    let mut meter = TerminalFuelMeter::with_allowance(16);
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        expected,
        "the caller's `right.flag` reads the subtree moved out of `left`"
    );
    let total = meter.usage().total_units();
    for split in 0..total {
        let mut execution = start();
        let mut meter = TerminalFuelMeter::with_allowance(split);
        assert!(matches!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        meter.replenish(total - split).unwrap();
        assert_eq!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn a_read_through_the_open_window_rejects_before_execution() {
    let mut module = window_module();
    // Reading the absent subtree between move and store must not replay:
    // admission itself rejects the stale read, so no bytes exist to run.
    let mut read = module.machines[0].blocks[0].operations[6].clone();
    read.id = operation_id(9);
    let OperationResult::Scalar(result) = &mut read.result else {
        unreachable!()
    };
    result.id = value_id(5);
    module.machines[0].blocks[0].operations.insert(1, read);
    assert_eq!(
        encode_module(&module),
        Err(terminal_codec::CodecError::InvalidModule(
            terminal_verifier::ModuleError::BorrowedStorageFieldAbsent {
                operation: operation_id(9),
                place: place_id(1),
            }
        ))
    );
}

#[test]
fn a_replaced_window_body_fails_the_sealed_proof() {
    // A module whose window names a different field is still well-formed —
    // but the proof section sealed for the original module does not admit it.
    let module = window_module();
    let mut replaced = module.clone();
    for operation in &mut replaced.machines[0].blocks[0].operations {
        match &mut operation.kind {
            OperationKind::MoveStructuralField { field, .. }
            | OperationKind::StoreStructuralField { field, .. } => *field = structural_field_id(2),
            _ => {}
        }
    }
    let bytes = encode_module(&replaced).expect("replaced module encodes");
    let proof = encode_proof_section(&module, &ProofBundle::default()).unwrap();
    let (arguments, scalar_fields) = structural_inputs(true, true);
    assert!(
        matches!(
            TerminalExecution::start_artifact(
                &bytes,
                &proof,
                &AdmissionProfile::default(),
                &[],
                terminal_interpreter::TerminalStructuralInputs {
                    arguments: &arguments,
                    scalar_fields: &scalar_fields,
                    ..Default::default()
                },
            ),
            Err(terminal_interpreter::TerminalArtifactInterpretError::ProofDecode(_))
        ),
        "a body with different window evidence is not admitted by the sealed proof"
    );
}
