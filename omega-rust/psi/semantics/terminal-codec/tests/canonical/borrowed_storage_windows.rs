use super::unit_fixture;
use crate::canonical::{operation_id, place_id, structural_field_id, structural_type_id, value_id};
use semantic_vocabulary::{ScalarType, StructuralPlaceKind};
use terminal_codec::{CodecError, decode_module, encode_module, semantic_fingerprint};
use terminal_psi::{
    BindingRelevance, Operation, OperationKind, OperationResult, RecordFieldInitializer,
    RecordFieldValue, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, ValueDeclaration,
};

fn cell() -> terminal_psi::StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "Cell".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: structural_field_id(1),
                identity: "flag".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            }],
        },
    }
}

fn envelope() -> terminal_psi::StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "Envelope".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                StructuralFieldDeclaration {
                    id: structural_field_id(1),
                    identity: "left".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(1)),
                },
                StructuralFieldDeclaration {
                    id: structural_field_id(2),
                    identity: "right".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(1)),
                },
            ],
        },
    }
}

fn structural_result(place: u64, structural_type: u64) -> OperationResult {
    OperationResult::Structural(StructuralOperationResult {
        place: place_id(place),
        structural_type: structural_type_id(structural_type),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    })
}

/// A machine that moves `left` out of its mutable-borrowed `Envelope`,
/// establishes a fresh `Cell`, and reseats the exact window.
fn window_module() -> terminal_psi::TerminalModule {
    let mut module = unit_fixture();
    module.structural_types = vec![cell(), envelope()];
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
    ];
    machine.parameters = vec![];
    machine.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: operation_id(1),
            result: structural_result(2, 1),
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
            result: structural_result(3, 1),
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
    ];
    module
}

#[test]
fn move_and_store_structural_field_round_trip() {
    let module = window_module();
    let bytes = encode_module(&module).expect("window module encodes");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(
        semantic_fingerprint(&decode_module(&bytes).unwrap()).unwrap(),
        semantic_fingerprint(&module).unwrap(),
    );
}

#[test]
fn the_field_spelling_is_part_of_the_recorded_evidence() {
    let mut bytes = encode_module(&window_module()).expect("window module encodes");
    let position = bytes
        .windows(4)
        .position(|window| window == b"left")
        .expect("encoded window field identity");
    bytes[position] = b'r'; // `left` -> `rift`: decode still succeeds, but the
    // decoded module no longer names a declared field.
    let tampered = decode_module(&bytes).expect("tampered bytes still decode");
    assert_ne!(
        semantic_fingerprint(&tampered).unwrap(),
        semantic_fingerprint(&window_module()).unwrap(),
        "a renamed window field is a different module"
    );
}

#[test]
fn a_move_sourced_outside_mutable_borrow_rejects_at_encoding() {
    let mut module = window_module();
    let OperationKind::MoveStructuralField { source, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    // The moved result place is owned, not a mutable-borrowed parameter.
    *source = place_id(3);
    assert_eq!(
        encode_module(&module),
        Err(CodecError::MalformedStructuralFoundation(
            "structural field move requires a mutable-borrowed parameter"
        ))
    );
}

#[test]
fn a_store_value_must_be_a_whole_owned_place() {
    let mut module = window_module();
    let OperationKind::StoreStructuralField { value, .. } =
        &mut module.machines[0].blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    value.access = StructuralAccess::SharedBorrow;
    assert_eq!(
        encode_module(&module),
        Err(CodecError::MalformedStructuralFoundation(
            "structural field store value must be a whole owned place"
        ))
    );
}
