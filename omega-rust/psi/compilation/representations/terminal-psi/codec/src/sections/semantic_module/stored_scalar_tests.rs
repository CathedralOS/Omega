//! Source-free scalar field membership encoding and exact operand joins.
use super::boundary_scalar_tests::boundary_parameter_module;
use super::module_wire::encode_raw;
use super::target_family_tests::identity_table;
use crate::{decode_module, encode_module, semantic_fingerprint};
use semantic_vocabulary::{
    OperationId, PlaceId, ScalarQualificationSetId, StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralTypeShape, TerminalIdentityIndex,
    TerminalModule,
};

fn module() -> TerminalModule {
    let mut module = boundary_parameter_module();
    module.identities = identity_table(9);
    let field = StructuralFieldId::new(1).unwrap();
    let owner = StructuralTypeId::new(1).unwrap();
    let place = PlaceId::new(1).unwrap();
    let producer = OperationId::new(1).unwrap();
    let value_type = module.machines[0].parameters[0].value_type();
    module
        .structural_types
        .push(terminal_psi::StructuralTypeDeclaration {
            quotient_carrier: None,
            id: owner,
            identity: TerminalIdentityIndex(8),
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape: StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::Packed,
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: field,
                    identity: TerminalIdentityIndex(9),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Scalar(value_type.scalar_type),
                    storage: None,
                }],
            },
        });
    module
        .scalar_qualifications
        .fields
        .push(terminal_psi::ScalarFieldQualification { field, value_type });
    module.machines[0]
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: place,
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer,
                structural_type: owner,
            },
        });
    let mut call = module.machines[0].blocks[0].operations.remove(0);
    call.id = OperationId::new(3).unwrap();
    let OperationKind::BoundaryCall { arguments, .. } = &mut call.kind else {
        unreachable!()
    };
    arguments[0] = ValueId::new(3).unwrap();
    module.machines[0].blocks[0].operations = vec![
        Operation {
            id: producer,
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place,
                structural_type: owner,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishRecord {
                fields: vec![terminal_psi::RecordFieldInitializer {
                    field,
                    value: terminal_psi::RecordFieldValue::Scalar {
                        value: ValueId::new(1).unwrap(),
                        range_obligation: None,
                    },
                }],
            },
        },
        Operation {
            id: OperationId::new(2).unwrap(),
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
                id: ValueId::new(3).unwrap(),
                scalar_type: value_type.scalar_type,
                qualifications: value_type.qualifications,
            }),
            kind: OperationKind::ScalarStructuralField {
                source: place,
                path: Vec::new(),
                field,
            },
        },
        call,
    ];
    module
}

#[test]
fn stored_scalar_membership_roundtrips_and_commits_its_field() {
    let original = module();
    assert_eq!(
        decode_module(&encode_module(&original).unwrap()).unwrap(),
        original
    );
    let mut changed = original.clone();
    changed.scalar_qualifications.fields.clear();
    assert_ne!(
        semantic_fingerprint(&original).unwrap(),
        semantic_fingerprint(&changed).unwrap()
    );
}

#[test]
fn stored_scalar_membership_rejects_detached_writes_and_reads() {
    let original = module();
    let mut changed = original.clone();
    changed.machines[0].blocks[0].operations[0].kind = OperationKind::EstablishZeroedStructural {};
    let failure = decode_module(&encode_raw(&changed).unwrap()).unwrap_err();
    assert!(
        failure
            .to_string()
            .contains("zero provisioning cannot establish scalar field membership"),
        "{failure}"
    );
    let mut changed = original.clone();
    changed.machines[0].parameters[0].qualifications = ScalarQualificationSetId::ZERO;
    changed.scalar_qualifications.returns[0].origins =
        vec![terminal_psi::ScalarReturnOrigin::Established { route: 0 }];
    let failure = decode_module(&encode_raw(&changed).unwrap()).unwrap_err();
    assert!(
        failure.to_string().contains("scalar field write lacks"),
        "{failure}"
    );
    let mut changed = original.clone();
    changed.scalar_qualifications.fields.clear();
    let failure = decode_module(&encode_raw(&changed).unwrap()).unwrap_err();
    assert!(
        failure.to_string().contains("scalar field read introduces"),
        "{failure}"
    );
    let mut changed = original.clone();
    changed.scalar_qualifications.fields[0].field = StructuralFieldId::new(99).unwrap();
    assert!(decode_module(&encode_raw(&changed).unwrap()).is_err());
    let mut changed = original.clone();
    changed.scalar_qualifications.fields[0]
        .value_type
        .qualifications = ScalarQualificationSetId::new(99);
    assert!(decode_module(&encode_raw(&changed).unwrap()).is_err());
    let mut changed = original;
    changed
        .scalar_qualifications
        .fields
        .push(changed.scalar_qualifications.fields[0]);
    assert!(decode_module(&encode_raw(&changed).unwrap()).is_err());
}

#[test]
fn zero_provisioning_checks_instantiated_scalar_memberships_only() {
    let mut empty_array = module();
    empty_array.identities = identity_table(10);
    let array = StructuralTypeId::new(2).unwrap();
    let mut declaration = empty_array.structural_types[0].clone();
    declaration.id = array;
    declaration.identity = TerminalIdentityIndex(10);
    declaration.shape = StructuralTypeShape::FixedArray {
        element: StructuralTypeId::new(1).unwrap(),
        length: 0,
    };
    empty_array.structural_types.push(declaration);
    empty_array.machines[0].blocks[0].operations.truncate(1);
    empty_array.machines[0].blocks[0].operations[0].kind =
        OperationKind::EstablishZeroedStructural {};
    let OperationResult::Structural(result) =
        &mut empty_array.machines[0].blocks[0].operations[0].result
    else {
        unreachable!()
    };
    result.structural_type = array;
    let semantic_vocabulary::StructuralPlaceKind::OperationResult {
        structural_type, ..
    } = &mut empty_array.machines[0].structural_places[0].kind
    else {
        unreachable!()
    };
    *structural_type = array;
    assert_eq!(
        decode_module(&encode_module(&empty_array).unwrap()).unwrap(),
        empty_array
    );
    let StructuralTypeShape::FixedArray { length, .. } = &mut empty_array.structural_types[1].shape
    else {
        unreachable!()
    };
    *length = 1;
    let failure = decode_module(&encode_raw(&empty_array).unwrap()).unwrap_err();
    assert!(
        failure
            .to_string()
            .contains("zero provisioning cannot establish scalar field membership"),
        "{failure}"
    );

    let mut inactive = module();
    let StructuralTypeShape::Record { fields, .. } = inactive.structural_types[0].shape.clone()
    else {
        unreachable!()
    };
    inactive.structural_types[0].shape = StructuralTypeShape::Sum {
        cases: vec![
            terminal_psi::StructuralCaseDeclaration {
                id: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
                identity: TerminalIdentityIndex(8),
                fields: Vec::new(),
            },
            terminal_psi::StructuralCaseDeclaration {
                id: semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
                identity: TerminalIdentityIndex(9),
                fields,
            },
        ],
    };
    inactive.machines[0].blocks[0].operations.truncate(1);
    inactive.machines[0].blocks[0].operations[0].kind = OperationKind::EstablishZeroedStructural {};
    assert_eq!(
        decode_module(&encode_module(&inactive).unwrap()).unwrap(),
        inactive
    );
    let StructuralTypeShape::Sum { cases } = &mut inactive.structural_types[0].shape else {
        unreachable!()
    };
    cases[0].fields = std::mem::take(&mut cases[1].fields);
    let failure = decode_module(&encode_raw(&inactive).unwrap()).unwrap_err();
    assert!(
        failure
            .to_string()
            .contains("zero provisioning cannot establish scalar field membership"),
        "{failure}"
    );
}

fn borrowed_field_module() -> TerminalModule {
    let mut original = module();
    original.identities = identity_table(11);
    let primitive = StructuralTypeId::new(2).unwrap();
    let reference = StructuralTypeId::new(3).unwrap();
    let borrowed = PlaceId::new(2).unwrap();
    let carrier = original.machines[0].parameters[0].scalar_type;
    let mut declaration = original.structural_types[0].clone();
    declaration.id = primitive;
    declaration.identity = TerminalIdentityIndex(10);
    declaration.shape = StructuralTypeShape::PrimitiveScalar(carrier);
    original.structural_types.push(declaration.clone());
    declaration.id = reference;
    declaration.identity = TerminalIdentityIndex(11);
    declaration.no_code_affine_cleanup = false;
    declaration.shape = StructuralTypeShape::Reference {
        referent: primitive,
        access: terminal_psi::StructuralAccess::MutableBorrow,
    };
    original.structural_types.push(declaration);
    let result = terminal_psi::StructuralOperationResult {
        place: borrowed,
        structural_type: reference,
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    };
    original.machines[0]
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: borrowed,
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: OperationId::new(2).unwrap(),
                structural_type: reference,
            },
        });
    let operation = |id, result, kind| Operation {
        id: OperationId::new(id).unwrap(),
        result,
        kind,
        static_reach_binding: None,
        suspension_crossing: None,
    };
    let borrowed_operations = vec![
        operation(
            2,
            OperationResult::Structural(result),
            OperationKind::EstablishReference {
                source: terminal_psi::StructuralArgument {
                    place: PlaceId::new(1).unwrap(),
                    access: terminal_psi::StructuralAccess::MutableBorrow,
                    path: vec![terminal_psi::StructuralPathSegment::Field(
                        TerminalIdentityIndex(9),
                    )],
                },
                storage: None,
            },
        ),
        operation(
            3,
            OperationResult::Scalar(terminal_psi::ValueDeclaration {
                id: ValueId::new(4).unwrap(),
                scalar_type: carrier,
                qualifications: ScalarQualificationSetId::ZERO,
            }),
            OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Signed(0),
            },
        ),
        operation(
            4,
            OperationResult::Unit,
            OperationKind::WriteOnlyPrimitiveStore {
                destination: borrowed,
                path: vec![terminal_psi::StructuralPathSegment::Referent],
                value: ValueId::new(1).unwrap(),
                range_obligation: None,
            },
        ),
        operation(
            5,
            OperationResult::Unit,
            OperationKind::ReleaseReference {
                source: borrowed,
                path: Vec::new(),
            },
        ),
    ];
    original.machines[0].blocks[0]
        .operations
        .splice(1..1, borrowed_operations);
    for (ordinal, operation) in original.machines[0].blocks[0]
        .operations
        .iter_mut()
        .enumerate()
    {
        operation.id = OperationId::new(ordinal as u64 + 1).unwrap();
    }
    original
}

#[test]
fn writable_scalar_field_reference_cannot_erase_receiving_membership() {
    let original = borrowed_field_module();
    let failure = decode_module(&encode_raw(&original).unwrap()).unwrap_err();
    assert!(
        failure
            .to_string()
            .contains("primitive reference loses scalar field membership"),
        "{failure}"
    );
}

#[test]
fn record_reference_keeps_its_scalar_field_contract() {
    let mut original = borrowed_field_module();
    original.structural_types[2].shape = StructuralTypeShape::Reference {
        referent: StructuralTypeId::new(1).unwrap(),
        access: terminal_psi::StructuralAccess::MutableBorrow,
    };
    let OperationKind::EstablishReference { source, .. } =
        &mut original.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    source.path.clear();
    let mut read = original.machines[0].blocks[0].operations[5].clone();
    read.kind = OperationKind::PrimitiveScalarRead {
        source: PlaceId::new(2).unwrap(),
        path: vec![
            terminal_psi::StructuralPathSegment::Referent,
            terminal_psi::StructuralPathSegment::Field(TerminalIdentityIndex(9)),
        ],
    };
    let operations = &mut original.machines[0].blocks[0].operations;
    let release = operations[4].clone();
    let call = operations[6].clone();
    operations.truncate(2);
    operations.extend([read, release, call]);
    for (ordinal, operation) in operations.iter_mut().enumerate() {
        operation.id = OperationId::new(ordinal as u64 + 1).unwrap();
    }
    assert_eq!(
        decode_module(&encode_module(&original).unwrap()).unwrap(),
        original
    );
}

#[test]
fn shared_scalar_observation_does_not_weaken_writable_membership() {
    let mut original = borrowed_field_module();
    original.structural_types[2].shape = StructuralTypeShape::Reference {
        referent: StructuralTypeId::new(2).unwrap(),
        access: terminal_psi::StructuralAccess::SharedBorrow,
    };
    let OperationKind::EstablishReference { source, .. } =
        &mut original.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    source.access = terminal_psi::StructuralAccess::SharedBorrow;
    let operations = &mut original.machines[0].blocks[0].operations;
    operations[2].kind = OperationKind::PrimitiveScalarRead {
        source: PlaceId::new(2).unwrap(),
        path: vec![terminal_psi::StructuralPathSegment::Referent],
    };
    operations.remove(3);
    for (ordinal, operation) in operations.iter_mut().enumerate() {
        operation.id = OperationId::new(ordinal as u64 + 1).unwrap();
    }
    assert_eq!(
        decode_module(&encode_module(&original).unwrap()).unwrap(),
        original
    );
}

#[test]
fn direct_primitive_call_loan_cannot_erase_field_membership() {
    use semantic_vocabulary::{ContractId, MachineId};
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
        TerminalMachineResult, Terminator,
    };
    let mut original = borrowed_field_module();
    let mut callee = original.machines[0].clone();
    callee.id = MachineId::new(2).unwrap();
    callee.declaration = None;
    callee.result = TerminalMachineResult::Unit;
    callee.parameters[0].qualifications = ScalarQualificationSetId::ZERO;
    callee.contract.id = ContractId::new(2).unwrap();
    callee.contract.ensures.clear();
    callee.structural_parameters = vec![StructuralParameterDeclaration {
        place: PlaceId::new(1).unwrap(),
        position: 0,
        structural_type: StructuralTypeId::new(2).unwrap(),
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    callee.structural_places = vec![terminal_psi::StructuralPlaceDeclaration {
        id: PlaceId::new(1).unwrap(),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter { position: 0 },
    }];
    let mut write = callee.blocks[0].operations[3].clone();
    write.id = OperationId::new(1).unwrap();
    write.kind = OperationKind::WriteOnlyPrimitiveStore {
        destination: PlaceId::new(1).unwrap(),
        path: Vec::new(),
        value: ValueId::new(1).unwrap(),
        range_obligation: None,
    };
    callee.blocks[0].operations = vec![write];
    callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: semantic_vocabulary::EdgeId::new(1).unwrap(),
        cleanup_actions: Vec::new(),
    };
    original.machines.push(callee);
    original.machines[0].structural_places.truncate(1);
    let operations = &mut original.machines[0].blocks[0].operations;
    operations.remove(4);
    operations.remove(1);
    operations[2].kind = OperationKind::CallUnit {
        callee: MachineId::new(2).unwrap(),
        arguments: vec![ValueId::new(4).unwrap()],
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: PlaceId::new(1).unwrap(),
            access: StructuralAccess::MutableBorrow,
            path: vec![terminal_psi::StructuralPathSegment::Field(
                TerminalIdentityIndex(9),
            )],
        }],
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    for (ordinal, operation) in operations.iter_mut().enumerate() {
        operation.id = OperationId::new(ordinal as u64 + 1).unwrap();
    }
    let failure = decode_module(&encode_raw(&original).unwrap()).unwrap_err();
    assert!(
        failure
            .to_string()
            .contains("primitive reference loses scalar field membership"),
        "{failure}"
    );
}

fn reinterpreted_record_module(
    access: terminal_psi::StructuralAccess,
    introduces: bool,
) -> TerminalModule {
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
    use terminal_psi::{ReferenceStorageView, StructuralPathSegment};
    let mut original = borrowed_field_module();
    original.identities = identity_table(13);
    let bare = StructuralTypeId::new(4).unwrap();
    let bare_field = StructuralFieldId::new(2).unwrap();
    let mut declaration = original.structural_types[0].clone();
    declaration.id = bare;
    declaration.identity = TerminalIdentityIndex(12);
    let StructuralTypeShape::Record { fields, .. } = &mut declaration.shape else {
        unreachable!()
    };
    fields[0].id = bare_field;
    fields[0].identity = TerminalIdentityIndex(13);
    original.structural_types.push(declaration);
    original.structural_types[2].shape = StructuralTypeShape::Reference {
        referent: if introduces {
            StructuralTypeId::new(1).unwrap()
        } else {
            bare
        },
        access,
    };
    let mut operations = original.machines[0].blocks[0].operations.clone();
    let mut record = operations.remove(0);
    if introduces {
        let OperationResult::Structural(result) = &mut record.result else {
            unreachable!()
        };
        result.structural_type = bare;
        record.kind = OperationKind::EstablishRecord {
            fields: vec![terminal_psi::RecordFieldInitializer {
                field: bare_field,
                value: terminal_psi::RecordFieldValue::Scalar {
                    value: ValueId::new(4).unwrap(),
                    range_obligation: None,
                },
            }],
        };
    }
    let mut reference = operations.remove(0);
    let OperationKind::EstablishReference { source, storage } = &mut reference.kind else {
        unreachable!()
    };
    source.path.clear();
    source.access = access;
    *storage = Some(ReferenceStorageView {
        byte_offset: ValueId::new(5).unwrap(),
        stored_scalar: None,
    });
    let constant = operations.remove(0);
    let mut write = operations.remove(0);
    let release = operations.remove(0);
    let mut read = operations.remove(0);
    let call = operations.remove(0);
    if introduces {
        read.kind = OperationKind::PrimitiveScalarRead {
            source: PlaceId::new(2).unwrap(),
            path: vec![
                StructuralPathSegment::Referent,
                StructuralPathSegment::Field(TerminalIdentityIndex(9)),
            ],
        };
    } else if access == terminal_psi::StructuralAccess::SharedBorrow {
        write.result = OperationResult::Scalar(terminal_psi::ValueDeclaration {
            id: ValueId::new(6).unwrap(),
            scalar_type: original.machines[0].parameters[0].scalar_type,
            qualifications: ScalarQualificationSetId::ZERO,
        });
        write.kind = OperationKind::PrimitiveScalarRead {
            source: PlaceId::new(2).unwrap(),
            path: vec![
                StructuralPathSegment::Referent,
                StructuralPathSegment::Field(TerminalIdentityIndex(13)),
            ],
        };
    } else {
        write.kind = OperationKind::StructuralScalarFieldStore {
            destination: PlaceId::new(2).unwrap(),
            path: vec![StructuralPathSegment::Referent],
            field: bare_field,
            value: ValueId::new(4).unwrap(),
            range_obligation: None,
        };
    }
    let offset = Operation {
        id: OperationId::new(2).unwrap(),
        result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
            id: ValueId::new(5).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            qualifications: ScalarQualificationSetId::ZERO,
        }),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(0),
        },
        static_reach_binding: None,
        suspension_crossing: None,
    };
    let body = &mut original.machines[0].blocks[0].operations;
    *body = vec![constant, offset, record, reference];
    if introduces {
        body.extend([read, release, call]);
    } else {
        body.extend([write, release, read, call]);
    }
    for (ordinal, operation) in body.iter_mut().enumerate() {
        operation.id = OperationId::new(ordinal as u64 + 1).unwrap();
    }
    for place in &mut original.machines[0].structural_places {
        let semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } = &mut place.kind
        else {
            unreachable!()
        };
        *producer = OperationId::new(if place.id == PlaceId::new(1).unwrap() {
            3
        } else {
            4
        })
        .unwrap();
        if introduces && place.id == PlaceId::new(1).unwrap() {
            *structural_type = bare;
        }
    }
    original
}

#[test]
fn aggregate_storage_view_cannot_erase_or_introduce_scalar_membership() {
    for (access, introduces) in [
        (terminal_psi::StructuralAccess::SharedBorrow, true),
        (terminal_psi::StructuralAccess::MutableBorrow, false),
    ] {
        let changed = reinterpreted_record_module(access, introduces);
        let failure = decode_module(&encode_raw(&changed).unwrap()).unwrap_err();
        assert!(
            failure
                .to_string()
                .contains("storage reinterpretation lacks scalar field membership correspondence"),
            "{access:?}, introduces={introduces}: {failure}"
        );
    }
}

#[test]
fn shared_storage_view_may_observe_without_field_membership() {
    let original = reinterpreted_record_module(terminal_psi::StructuralAccess::SharedBorrow, false);
    assert_eq!(
        decode_module(&encode_module(&original).unwrap()).unwrap(),
        original
    );
}

#[test]
fn scalar_field_catalog_checks_every_matching_declaration() {
    let mut original = module();
    original.identities = identity_table(10);
    let mut other = original.structural_types[0].clone();
    other.id = StructuralTypeId::new(2).unwrap();
    other.identity = TerminalIdentityIndex(10);
    original.structural_types.push(other);
    assert_eq!(
        decode_module(&encode_module(&original).unwrap()).unwrap(),
        original
    );
    let scalar = original.machines[0].parameters[0].scalar_type;
    for mutation in 0..3 {
        for owner in 0..2 {
            let mut changed = original.clone();
            let StructuralTypeShape::Record { fields, .. } =
                &mut changed.structural_types[owner].shape
            else {
                unreachable!()
            };
            match mutation {
                0 => {
                    fields[0].field_type = terminal_psi::StructuralFieldType::Scalar(
                        semantic_vocabulary::ScalarType::Boolean,
                    )
                }
                1 => fields[0].field_type = terminal_psi::StructuralFieldType::Atomic(scalar),
                _ => fields[0].relevance = terminal_psi::BindingRelevance::Erased,
            }
            let failure = decode_module(&encode_raw(&changed).unwrap()).unwrap_err();
            assert!(
                failure
                    .to_string()
                    .contains("scalar field membership changes its declared carrier"),
                "mutation={mutation}, owner={owner}: {failure}"
            );
        }
    }
}
