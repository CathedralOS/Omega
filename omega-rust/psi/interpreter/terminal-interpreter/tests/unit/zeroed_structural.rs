//! Zeroed structural establishment materializes every scalar leaf of a
//! zero-constructible aggregate root: record scalar fields, nested record
//! members and primitive fixed-array elements all read back as zero.
use super::{
    AdmissionProfile, BindingRelevance, IntegerSign, IntegerType, IntegerValue, Operation,
    OperationKind, OperationResult, ProofBundle, ScalarType, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalFuelMeter, TerminalMachineResult,
    TerminalModule, TerminalScalarValue, Terminator, ValueDeclaration, block_id, contract_id,
    decode_module, edge_id, encode_module, encode_proof_section, operation_id, place_id,
    structural_field_id, structural_type_id, unit_module, value_id, verify_module,
};
use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralPlaceKind};
use terminal_interpreter::{AcceptTerminalEffects, TerminalExecution, TerminalStructuralInputs};

fn integer() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

fn scalar(ordinal: u64) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(ordinal),
        scalar_type: integer(),
    }
}

fn field(id: u64, name: &str, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: structural_field_id(id),
        identity: name.into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    }
}

fn structural_result(place: u64, structural_type: u64) -> OperationResult {
    OperationResult::Structural(StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: place_id(place),
        structural_type: structural_type_id(structural_type),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    })
}

fn integer_field(ordinal: u64, path: Vec<CanonicalStructuralPathSegment>, field: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(ordinal),
        result: OperationResult::Scalar(scalar(ordinal)),
        kind: OperationKind::IntegerStructuralField {
            path,
            source: place_id(1),
            field: structural_field_id(field),
        },
    }
}

fn add(ordinal: u64, left: u64, right: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(ordinal),
        result: OperationResult::Scalar(scalar(ordinal)),
        kind: OperationKind::WrappingIntegerAdd {
            left: value_id(left),
            right: value_id(right),
        },
    }
}

/// `Bag` nests every leaf shape a zeroed establishment owes: a fixed array of
/// records (`pts`), a nested record (`inner`), and direct scalar fields
/// (`count`, `tail`).
fn bag_module() -> TerminalModule {
    let mut module = unit_module();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: structural_type_id(1),
            identity: "Point".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(1, "x", StructuralFieldType::Scalar(integer())),
                    field(2, "y", StructuralFieldType::Scalar(integer())),
                ],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(2),
            identity: "Inner".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(1, "code", StructuralFieldType::Scalar(integer())),
                    field(2, "flag", StructuralFieldType::Scalar(ScalarType::Boolean)),
                ],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(3),
            identity: "PointArray".into(),
            shape: StructuralTypeShape::FixedArray {
                element: structural_type_id(1),
                length: 2,
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(4),
            identity: "Bag".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(
                        1,
                        "pts",
                        StructuralFieldType::Structural(structural_type_id(3)),
                    ),
                    field(
                        2,
                        "inner",
                        StructuralFieldType::Structural(structural_type_id(2)),
                    ),
                    field(3, "count", StructuralFieldType::Scalar(integer())),
                    field(4, "tail", StructuralFieldType::Scalar(integer())),
                ],
            },
        },
    ];
    let caller = &mut module.machines[0];
    caller.result = TerminalMachineResult::Scalar(scalar(9));
    caller.structural_places = vec![StructuralPlaceDeclaration {
        id: place_id(1),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(1),
            structural_type: structural_type_id(4),
        },
    }];
    caller.blocks[0].id = block_id(1);
    caller.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(1),
            result: structural_result(1, 4),
            kind: OperationKind::EstablishZeroedStructural {},
        },
        // pts[1].x
        integer_field(
            2,
            vec![
                CanonicalStructuralPathSegment::Field(structural_field_id(1)),
                CanonicalStructuralPathSegment::FixedIndex(1),
            ],
            1,
        ),
        // inner.code
        integer_field(
            3,
            vec![CanonicalStructuralPathSegment::Field(structural_field_id(
                2,
            ))],
            1,
        ),
        // count
        integer_field(4, Vec::new(), 3),
        // tail
        integer_field(5, Vec::new(), 4),
        add(6, 2, 3),
        add(7, 6, 4),
        add(8, 7, 5),
    ];
    caller.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(8),
        cleanup_actions: Vec::new(),
    };
    caller.contract.id = contract_id(1);
    module
}

#[test]
fn zeroed_structural_establishment_materializes_every_scalar_leaf_as_zero() {
    let module = bag_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let semantic = encode_module(&module).unwrap();
    assert_eq!(decode_module(&semantic).unwrap(), module);
    let proof = encode_proof_section(&module, &ProofBundle::default()).unwrap();
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .unwrap();
    match execution
        .resume(
            &mut TerminalFuelMeter::unbounded(),
            &mut AcceptTerminalEffects,
        )
        .unwrap()
    {
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Integer {
                value: IntegerValue::Unsigned(0),
                ..
            },
        )) => {}
        other => panic!("zeroed leaf sum is not zero: {other:?}"),
    }
}
