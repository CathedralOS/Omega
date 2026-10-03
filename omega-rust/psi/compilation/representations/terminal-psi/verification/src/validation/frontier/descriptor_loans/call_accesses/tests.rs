use super::*;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, ObligationId, OperationId, PlaceId, ScalarType, StructuralFieldId,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, Block, MachineContract, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration,
};

fn machine(identity: u64, operations: Vec<Operation>) -> TerminalMachine {
    let place = PlaceId::new(1).unwrap();
    let entry = BlockId::new(1).unwrap();
    TerminalMachine {
        id: MachineId::new(identity).unwrap(),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type: StructuralTypeId::new(1).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        entry_claims: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![Block {
            id: entry,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations,
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(1).unwrap(),
            crash_routes: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn operation(kind: OperationKind) -> Operation {
    Operation {
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Unit,
        kind,
        static_reach_binding: None,
        suspension_crossing: None,
    }
}

fn call(callee: u64, path: Vec<StructuralPathSegment>) -> Operation {
    operation(OperationKind::CallUnit {
        callee: MachineId::new(callee).unwrap(),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: PlaceId::new(1).unwrap(),
            path,
            access: StructuralAccess::MutableBorrow,
        }],
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    })
}

fn module(callee: TerminalMachine) -> TerminalModule {
    let mut module = TerminalModule::for_entry(callee.id);
    module.structural_types = vec![StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).unwrap(),
        identity: "test::Pair".into(),
        shape: StructuralTypeShape::Record {
            fields: ["left", "right"]
                .into_iter()
                .enumerate()
                .map(|(index, identity)| StructuralFieldDeclaration {
                    id: StructuralFieldId::new(index as u64 + 1).unwrap(),
                    identity: identity.into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                })
                .collect(),
        },
    }];
    module.machines.push(callee);
    module
}

fn store(path: Vec<StructuralPathSegment>) -> Operation {
    operation(OperationKind::StructuralScalarFieldStore {
        destination: PlaceId::new(1).unwrap(),
        path,
        field: StructuralFieldId::new(2).unwrap(),
        value: ValueId::new(1).unwrap(),
        range_obligation: None,
    })
}

#[test]
fn descriptor_call_effects_preserve_projected_writes_and_reads() {
    let mut read = operation(OperationKind::BooleanStructuralField {
        source: PlaceId::new(1).unwrap(),
        path: Vec::new(),
        field: StructuralFieldId::new(1).unwrap(),
    });
    read.id = OperationId::new(2).unwrap();
    let module = module(machine(1, vec![store(Vec::new()), read]));
    let summary = CallAccesses::new(&module);
    let caller = machine(2, Vec::new());
    let effects = summary.operation(&module, &caller, &call(1, Vec::new()), (caller.entry, 0));
    assert_eq!(effects.len(), 2);
    assert!(effects.iter().any(|effect| effect.exclusive
        && effect.projection.path == vec![StructuralPathSegment::Field("right".into())]));
    assert!(effects.iter().any(|effect| !effect.exclusive
        && effect.projection.path == vec![StructuralPathSegment::Field("left".into())]));
    let held = borrowed_storage::Region {
        root: Some(PlaceId::new(1).unwrap()),
        projection: Projection::new(&[StructuralPathSegment::Field("left".into())]),
    };
    assert!(!held.overlaps(&borrowed_storage::Region {
        root: Some(effects[0].place),
        projection: effects[0].projection.clone()
    }));
}

#[test]
fn descriptor_call_effects_do_not_turn_metadata_or_zero_actual_boundaries_into_writes() {
    let length = operation(OperationKind::ElementViewLength {
        source: PlaceId::new(1).unwrap(),
    });
    let mut boundary = operation(OperationKind::BoundaryCall {
        boundary: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        completion_receipts: Vec::new(),
        requirement_obligations: Vec::new(),
    });
    boundary.id = OperationId::new(2).unwrap();
    let mut module = module(machine(1, vec![length, boundary]));
    // The adapter operates on current declarations; the full verifier owns
    // scalar result/boundary contract validity independently.
    module.structural_types[0].shape = StructuralTypeShape::ElementView {
        element: StructuralTypeId::new(2).unwrap(),
    };
    let caller = machine(2, Vec::new());
    assert!(
        CallAccesses::new(&module)
            .operation(&module, &caller, &call(1, Vec::new()), (caller.entry, 0))
            .is_empty()
    );
}

#[test]
fn descriptor_call_effects_strip_callee_selectors_but_keep_actual_coordinates() {
    let selector = StructuralPathSegment::RuntimeIndex {
        index: ValueId::new(9).unwrap(),
        obligation: ObligationId::new(9).unwrap(),
    };
    let mut module = module(machine(1, vec![store(vec![selector.clone()])]));
    let record = module.structural_types[0].clone();
    module.structural_types[0].shape = StructuralTypeShape::FixedArray {
        element: StructuralTypeId::new(2).unwrap(),
        length: 4,
    };
    module.structural_types.push(StructuralTypeDeclaration {
        id: StructuralTypeId::new(2).unwrap(),
        ..record
    });
    let caller = machine(2, Vec::new());
    let summaries = CallAccesses::new(&module);
    let effects = summaries.operation(&module, &caller, &call(1, Vec::new()), (caller.entry, 0));
    assert_eq!(effects.len(), 1);
    assert!(effects[0].projection.path.is_empty());
    assert!(!effects[0].projection.exact);
    assert!(effects[0].projection.captured_at.is_none());
    let effects = summaries.operation(
        &module,
        &caller,
        &call(1, vec![selector.clone()]),
        (caller.entry, 0),
    );
    assert_eq!(effects[0].projection.path, vec![selector]);
}

#[test]
fn descriptor_call_effects_keep_recursive_or_missing_callees_conservative() {
    let module = module(machine(1, vec![call(1, Vec::new())]));
    let summaries = CallAccesses::new(&module);
    assert!(summaries.summaries[&MachineId::new(1).unwrap()].is_none());
    let caller = machine(2, Vec::new());
    for callee in [1, 99] {
        let effects = summaries.operation(
            &module,
            &caller,
            &call(callee, Vec::new()),
            (caller.entry, 0),
        );
        assert_eq!(effects.len(), 1);
        assert!(effects[0].exclusive);
        assert!(effects[0].projection.path.is_empty());
    }
}

#[test]
fn captured_byte_window_allows_descriptor_copy_but_pins_original_backing() {
    use super::super::DescriptorLoans;
    use terminal_psi::{
        ByteSequenceCarrier, RecordFieldInitializer, RecordFieldValue, StructuralOperationResult,
    };

    let root = PlaceId::new(1).unwrap();
    let descriptor = PlaceId::new(2).unwrap();
    let record = PlaceId::new(3).unwrap();
    let byte_type = StructuralTypeId::new(4).unwrap();
    let array_type = StructuralTypeId::new(2).unwrap();
    let view_type = StructuralTypeId::new(3).unwrap();
    let record_type = StructuralTypeId::new(5).unwrap();
    let carrier = ByteSequenceCarrier::BorrowedView {
        access: Some(StructuralAccess::SharedBorrow),
    };
    let mut module = module(machine(1, Vec::new()));
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    for field in fields {
        field.field_type = StructuralFieldType::Structural(array_type);
    }
    module.structural_types.extend([
        StructuralTypeDeclaration {
            id: array_type,
            identity: "Bytes".into(),
            shape: StructuralTypeShape::FixedArray {
                element: byte_type,
                length: 4,
            },
        },
        StructuralTypeDeclaration {
            id: view_type,
            identity: "ByteView".into(),
            shape: StructuralTypeShape::ByteSequence(carrier),
        },
        StructuralTypeDeclaration {
            id: byte_type,
            identity: "Byte".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    8,
                )
                .unwrap(),
            )),
        },
        StructuralTypeDeclaration {
            id: record_type,
            identity: "Sample".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(3).unwrap(),
                    identity: "bytes".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::ByteSequence(carrier),
                }],
            },
        },
    ]);
    let result = |place, structural_type, multiplicity| {
        OperationResult::Structural(StructuralOperationResult {
            place,
            structural_type,
            multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        })
    };
    let mut establish = operation(OperationKind::EstablishElementView {
        region: terminal_psi::ElementViewRegion::WholeCollection,
        destination: descriptor,
        source: StructuralArgument {
            place: root,
            path: vec![
                StructuralPathSegment::Field("left".into()),
                StructuralPathSegment::FixedByteRange { start: 1, end: 3 },
            ],
            access: StructuralAccess::SharedBorrow,
        },
        element: byte_type,
    });
    establish.result = result(descriptor, view_type, StructuralMultiplicity::Unrestricted);
    let mut capture = operation(OperationKind::EstablishRecord {
        fields: vec![RecordFieldInitializer {
            field: StructuralFieldId::new(3).unwrap(),
            value: RecordFieldValue::Structural(StructuralArgument {
                place: descriptor,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }),
        }],
    });
    capture.id = OperationId::new(2).unwrap();
    capture.result = result(record, record_type, StructuralMultiplicity::Affine);
    let mut write = operation(OperationKind::WriteOnlyPrimitiveStore {
        destination: root,
        path: vec![
            StructuralPathSegment::Field("right".into()),
            StructuralPathSegment::FixedIndex(1),
        ],
        value: ValueId::new(1).unwrap(),
    });
    write.id = OperationId::new(3).unwrap();
    let mut observe = call(99, Vec::new());
    observe.id = OperationId::new(4).unwrap();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut observe.kind
    else {
        unreachable!()
    };
    structural_arguments[0].place = record;
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    let caller = &mut module.machines[0];
    for producer in [&establish, &capture] {
        let result = producer.result.structural().unwrap();
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: result.place,
            kind: StructuralPlaceKind::OperationResult {
                producer: producer.id,
                structural_type: result.structural_type,
            },
        });
    }
    caller.blocks[0].operations = vec![establish, capture, write, observe];
    let caller = &module.machines[0];
    // This test isolates current-operation access and cohort reconstruction.
    // The corpus encoder supplies full declaration, proof and native replay.
    let loans = DescriptorLoans::new(&module, caller);
    let capture = &caller.blocks[0].operations[1];
    let write = &caller.blocks[0].operations[2];
    assert_eq!(loans.disturbed(&module, caller, capture, &[]), None);
    assert_eq!(loans.disturbed(&module, caller, write, &[]), None);
    let mut overlapping = write.clone();
    let OperationKind::WriteOnlyPrimitiveStore { path, .. } = &mut overlapping.kind else {
        unreachable!()
    };
    path[0] = StructuralPathSegment::Field("left".into());
    assert_eq!(
        loans.disturbed(&module, caller, &overlapping, &[]),
        Some(root)
    );
    assert_eq!(
        loans.disturbed(&module, caller, capture, &[root]),
        Some(root)
    );
}
