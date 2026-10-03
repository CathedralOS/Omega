//! Returned permission must preserve its parent through arbitrary ordinary work.
use super::{
    AdmissionProfile, Block, BoundaryMachineDeclaration, ContractClause, ModuleError, Operation,
    OperationKind, OperationResult, PlaceId, ProofBundle, Proposition, ScalarTerm, ScalarType,
    StructuralAccess, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralPathSegment, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, block_id, boundary_id,
    contract_id, edge_id, empty_contract, machine_id, obligation_id, operation_id, place_id,
    signed_i8, structural_type_id, validate_module, value_id, verify_module,
    write_only_primitive_store_module,
};
use terminal_psi::{StructuralOperationResult, StructuralReferenceResultSource};

fn argument(place: u64, through_reference: bool) -> StructuralArgument {
    StructuralArgument {
        place: place_id(place),
        path: if through_reference {
            vec![StructuralPathSegment::Referent]
        } else {
            Vec::new()
        },
        access: StructuralAccess::MutableBorrow,
    }
}

fn reference_result(place: u64) -> OperationResult {
    OperationResult::Structural(StructuralOperationResult {
        place: place_id(place),
        structural_type: structural_type_id(2),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    })
}

fn reference_place(place: u64, producer: u64) -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id: place_id(place),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(producer),
            structural_type: structural_type_id(2),
        },
    }
}

fn release(operation: u64, place: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(operation),
        result: OperationResult::Unit,
        kind: OperationKind::ReleaseReference {
            source: place_id(place),
        },
    }
}

fn establish(operation: u64, place: u64, source: StructuralArgument) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(operation),
        result: reference_result(place),
        kind: OperationKind::EstablishReference { source },
    }
}

fn relay_module(mark_before_return: bool) -> TerminalModule {
    let mut module = write_only_primitive_store_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "MutableI8Reference".into(),
        shape: StructuralTypeShape::Reference {
            referent: structural_type_id(1),
            access: StructuralAccess::MutableBorrow,
        },
    });
    let mut relay = module.machines[0].clone();
    relay.id = machine_id(2);
    relay.entry = block_id(2);
    relay.parameters.clear();
    relay.contract = empty_contract(contract_id(2));
    relay.structural_parameters[0].place = place_id(10);
    relay.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    relay.structural_places = vec![
        StructuralPlaceDeclaration {
            id: place_id(10),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(11),
            kind: StructuralPlaceKind::Result,
        },
        reference_place(12, 20),
    ];
    relay.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: place_id(11),
        structural_type: structural_type_id(2),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: vec![StructuralReferenceResultSource {
            path: Vec::new(),
            sources: vec![argument(10, false)],
        }],
    });
    relay.blocks[0].id = block_id(2);
    relay.blocks[0].operations = vec![establish(20, 12, argument(10, false))];
    if mark_before_return {
        relay.blocks[0].operations.splice(
            0..0,
            [
                Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: operation_id(18),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: value_id(10),
                        scalar_type: signed_i8(),
                        qualifications: Default::default(),
                    }),
                    kind: OperationKind::IntegerConstant {
                        value: semantic_vocabulary::IntegerValue::Signed(7),
                    },
                },
                Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: operation_id(19),
                    result: OperationResult::Unit,
                    kind: OperationKind::WriteOnlyPrimitiveStore {
                        path: Vec::new(),
                        destination: place_id(10),
                        value: value_id(10),
                    },
                },
            ],
        );
    }
    relay.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(2),
        source: place_id(12),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };

    let mut writer = module.machines[0].clone();
    writer.id = machine_id(3);
    writer.entry = block_id(3);
    writer.contract = empty_contract(contract_id(3));
    writer.parameters[0].id = value_id(30);
    writer.structural_parameters[0].place = place_id(30);
    writer.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    writer.structural_places[0].id = place_id(30);
    writer.blocks[0].id = block_id(3);
    writer.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(30),
        result: OperationResult::Unit,
        kind: OperationKind::WriteOnlyPrimitiveStore {
            path: Vec::new(),
            destination: place_id(30),
            value: value_id(30),
        },
    }];
    writer.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(3),
        trivial_affine_discards: Vec::new(),
    };

    let caller = &mut module.machines[0];
    caller.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    caller.structural_places.push(reference_place(2, 1));
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(99),
        scalar_type: signed_i8(),
        qualifications: Default::default(),
    });
    caller.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(1),
            result: reference_result(2),
            kind: OperationKind::CallStructural {
                callee: machine_id(2),
                structural_arguments: vec![argument(1, false)],
                claim_transfers: Vec::new(),
                returned_claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                selected_evidence: Vec::new(),
            },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(2),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                callee: machine_id(3),
                arguments: vec![value_id(1)],
                structural_arguments: vec![argument(2, true)],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        },
        release(3, 2),
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(4),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(2),
                scalar_type: signed_i8(),
                qualifications: Default::default(),
            }),
            kind: OperationKind::PrimitiveScalarRead {
                path: Vec::new(),
                source: place_id(1),
            },
        },
    ];
    caller.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(2),
        cleanup_actions: Vec::new(),
    };
    module.machines.extend([relay, writer]);
    module
}

fn rejects_custody(module: &TerminalModule) {
    assert!(
        matches!(
            validate_module(module),
            Err(ModuleError::InvalidReferenceCustody { .. })
        ),
        "{:?}",
        validate_module(module)
    );
}

fn returned_record_store(projected: bool, byte_store: bool) -> TerminalModule {
    use terminal_psi::{ByteSequenceCarrier, ByteSequencePredicate, ByteSequencePredicateSet};
    let mut module = relay_module(false);
    module.machines.truncate(2);
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    predicates.insert(ByteSequencePredicate::ValidUtf8);
    module.structural_types[0].identity = "Holder".into();
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![
            StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                identity: "sibling".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(signed_i8()),
            },
            StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(2).unwrap(),
                identity: "text".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::PredicateByteSequence {
                    carrier: ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                    predicates,
                },
            },
        ],
    };
    module.structural_types[1].identity = "MutableHolderReference".into();
    let caller = &mut module.machines[0];
    caller.blocks[0].operations.truncate(3);
    caller.blocks[0].operations[1].kind = OperationKind::StructuralScalarFieldStore {
        destination: place_id(2),
        path: vec![StructuralPathSegment::Referent],
        field: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
        value: value_id(1),
        range_obligation: None,
    };
    let Terminator::Return { value, .. } = &mut caller.blocks[0].terminator else {
        unreachable!()
    };
    *value = value_id(1);
    if projected {
        module.structural_types.extend([
            StructuralTypeDeclaration {
                id: structural_type_id(4),
                identity: "Holders".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: structural_type_id(1),
                    length: 2,
                },
            },
            StructuralTypeDeclaration {
                id: structural_type_id(5),
                identity: "Bag".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: semantic_vocabulary::StructuralFieldId::new(3).unwrap(),
                        identity: "holders".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(structural_type_id(4)),
                    }],
                },
            },
        ]);
        for machine in &mut module.machines {
            machine.structural_parameters[0].structural_type = structural_type_id(5);
        }
        let path = vec![
            StructuralPathSegment::Field("holders".into()),
            StructuralPathSegment::FixedIndex(1),
        ];
        let relay = &mut module.machines[1];
        let TerminalMachineResult::Structural(result) = &mut relay.result else {
            unreachable!()
        };
        result.reference_sources[0].sources[0].path = path.clone();
        let OperationKind::EstablishReference { source } = &mut relay.blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        source.path = path;
    }
    if byte_store {
        module.structural_types.push(StructuralTypeDeclaration {
            id: structural_type_id(3),
            identity: "LiteralBytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            }),
        });
        module
            .structural_types
            .sort_by_key(|declaration| declaration.id);
        let caller = &mut module.machines[0];
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(5),
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 0,
                structural_type: structural_type_id(3),
            },
        });
        caller.blocks[0].operations[1].kind = OperationKind::StructuralByteSequenceFieldStore {
            destination: place_id(2),
            path: vec![StructuralPathSegment::Referent],
            field: semantic_vocabulary::StructuralFieldId::new(2).unwrap(),
            source: place_id(5),
            length: value_id(10),
            obligation: obligation_id(1),
        };
        caller.blocks[0].operations.splice(
            1..1,
            [
                Operation {
                    id: operation_id(10),
                    result: OperationResult::Unit,
                    static_reach_binding: None,
                    suspension_crossing: None,
                    kind: OperationKind::EstablishByteSequenceLiteral {
                        destination: place_id(5),
                        bytes: b"Gate".to_vec(),
                        qualifications: Vec::new(),
                    },
                },
                Operation {
                    id: operation_id(11),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: value_id(10),
                        scalar_type: ScalarType::Integer(
                            semantic_vocabulary::IntegerType::new(
                                semantic_vocabulary::IntegerSign::Unsigned,
                                64,
                            )
                            .unwrap(),
                        ),
                        qualifications: Default::default(),
                    }),
                    static_reach_binding: None,
                    suspension_crossing: None,
                    kind: OperationKind::ByteSequenceLength {
                        source: place_id(5),
                    },
                },
            ],
        );
    }
    module
}

/// A source-free model of `let field = take selected.child; selected.child =
/// field` where `selected` is an ordinary mutable-record reference result.
/// The window stays keyed by the result carrier while the explicit Referent
/// crossing keeps every possible captured origin suspended.
fn returned_record_window() -> TerminalModule {
    let mut module = returned_record_store(false, false);
    let child_type = structural_type_id(3);
    let child_field = semantic_vocabulary::StructuralFieldId::new(4).unwrap();
    let child_value = semantic_vocabulary::StructuralFieldId::new(5).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: child_type,
        identity: "Child".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: child_value,
                identity: "value".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(signed_i8()),
            }],
        },
    });
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("Holder is a record")
    };
    fields.push(StructuralFieldDeclaration {
        id: child_field,
        identity: "child".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(child_type),
    });

    let caller = &mut module.machines[0];
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(3),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(2),
            structural_type: child_type,
        },
    });
    caller.blocks[0].operations[1] = Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(2),
        result: OperationResult::Structural(StructuralOperationResult {
            place: place_id(3),
            structural_type: child_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::MoveStructuralField {
            source: place_id(2),
            path: vec![StructuralPathSegment::Referent],
            field: Some(child_field),
        },
    };
    caller.blocks[0].operations.insert(
        2,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(5),
            result: OperationResult::Unit,
            kind: OperationKind::StoreStructuralField {
                destination: place_id(2),
                path: vec![StructuralPathSegment::Referent],
                field: Some(child_field),
                value: StructuralArgument {
                    place: place_id(3),
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
            },
        },
    );
    module
}

#[test]
fn returned_record_window_retains_exact_live_carrier_permission() {
    let module = returned_record_window();
    validate_module(&module).expect("a live returned reference may restore its exact child field");

    let mut observed_hole = module.clone();
    observed_hole.machines[0].blocks[0].operations.insert(
        2,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(6),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: place_id(2),
                path: vec![
                    StructuralPathSegment::Referent,
                    StructuralPathSegment::Field("child".into()),
                ],
                field: semantic_vocabulary::StructuralFieldId::new(5).unwrap(),
                value: value_id(1),
                range_obligation: None,
            },
        },
    );
    assert_eq!(
        validate_module(&observed_hole).map(|_| ()),
        Err(ModuleError::BorrowedStorageFieldAbsent {
            operation: operation_id(6),
            place: place_id(2),
        }),
        "a nested referent access cannot reach the displaced outer field"
    );

    let mut missing_crossing = module.clone();
    let OperationKind::MoveStructuralField { path, .. } =
        &mut missing_crossing.machines[0].blocks[0].operations[1].kind
    else {
        panic!("window extraction")
    };
    path.clear();
    validate_module(&missing_crossing).expect_err("the carrier is not its referent");

    let mut wrong_carrier = module.clone();
    let OperationKind::StoreStructuralField {
        destination, path, ..
    } = &mut wrong_carrier.machines[0].blocks[0].operations[2].kind
    else {
        panic!("window repair")
    };
    *destination = place_id(1);
    path.clear();
    validate_module(&wrong_carrier).expect_err("a captured origin cannot repair carrier debt");

    let mut released = module.clone();
    let release = released.machines[0].blocks[0].operations.remove(3);
    released.machines[0].blocks[0].operations.insert(1, release);
    validate_module(&released).expect_err("a released carrier cannot open a window");

    let mut suspended = module;
    let caller = &mut suspended.machines[0];
    caller.structural_places.push(reference_place(4, 9));
    caller.blocks[0]
        .operations
        .insert(1, establish(9, 4, argument(2, true)));
    validate_module(&suspended).expect_err("a live child keeps its returned parent suspended");
}

#[test]
fn returned_record_field_store_retains_mutable_reference_custody() {
    for projected in [false, true] {
        verify_module(
            &returned_record_store(projected, false),
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("scalar field store retains the returned record permission");
        let module = returned_record_store(projected, true);
        validate_module(&module).expect("byte replacement uses the same live mutable reference");
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err(),
            "the byte capacity obligation remains mandatory"
        );
    }
}

fn returned_record_read() -> TerminalModule {
    let mut module = returned_record_store(false, false);
    let caller = &mut module.machines[0];
    caller.blocks[0].operations[1].kind = OperationKind::IntegerStructuralField {
        source: place_id(2),
        path: Vec::new(),
        field: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
    };
    caller.blocks[0].operations[1].result = OperationResult::Scalar(ValueDeclaration {
        id: value_id(2),
        scalar_type: signed_i8(),
        qualifications: Default::default(),
    });
    let Terminator::Return { value, .. } = &mut caller.blocks[0].terminator else {
        unreachable!()
    };
    *value = value_id(2);
    module
}

#[test]
fn returned_record_field_read_requires_live_unsuspended_permission() {
    let original = returned_record_read();
    verify_module(
        &original,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a live mutable carrier permits field observation");

    let mut released = original.clone();
    released.machines[0].blocks[0].operations.swap(1, 2);
    rejects_custody(&released);

    let mut parent = original.clone();
    let OperationKind::IntegerStructuralField { source, .. } =
        &mut parent.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *source = place_id(1);
    rejects_custody(&parent);

    let mut child = original;
    let caller = &mut child.machines[0];
    caller.structural_places.push(reference_place(3, 9));
    caller.blocks[0]
        .operations
        .insert(1, establish(9, 3, argument(2, true)));
    assert_eq!(
        validate_module(&child).map(|_| ()),
        Err(ModuleError::InvalidReferenceCustody {
            machine: machine_id(1),
            reason: "field source reference remains suspended by a live loan",
        })
    );
    child.machines[0].blocks[0]
        .operations
        .insert(2, release(12, 3));
    verify_module(
        &child,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("ending the child restores the parent's read permission");
}

fn branch_selected_record_return() -> TerminalModule {
    let mut module = returned_record_store(true, false);
    let StructuralTypeShape::FixedArray { length, .. } = &mut module.structural_types[2].shape
    else {
        panic!("holder array");
    };
    *length = 3;
    let caller = &mut module.machines[0];
    caller.parameters.push(ValueDeclaration {
        id: value_id(50),
        scalar_type: ScalarType::Boolean,
        qualifications: Default::default(),
    });
    caller.blocks[0].operations[0].kind = OperationKind::CallStructuralWithScalarArguments {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        callee: machine_id(2),
        arguments: vec![value_id(50)],
        structural_arguments: vec![argument(1, false)],
        claim_transfers: Vec::new(),
        returned_claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let relay = &mut module.machines[1];
    relay.parameters.push(ValueDeclaration {
        id: value_id(60),
        scalar_type: ScalarType::Boolean,
        qualifications: Default::default(),
    });
    let TerminalMachineResult::Structural(result) = &mut relay.result else {
        panic!("reference result");
    };
    let mut first = result.reference_sources[0].sources[0].clone();
    first.path[1] = StructuralPathSegment::FixedIndex(0);
    result.reference_sources[0].sources.insert(0, first.clone());
    let mut second_arm = relay.blocks[0].clone();
    second_arm.id = block_id(3);
    let mut first_arm = second_arm.clone();
    first_arm.id = block_id(4);
    first_arm.operations = vec![establish(21, 13, first)];
    first_arm.terminator = Terminator::ReturnStructural {
        edge: edge_id(4),
        source: place_id(13),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    relay.structural_places.push(reference_place(13, 21));
    let successor = |edge, target| SuccessorEdge {
        edge: edge_id(edge),
        target: block_id(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    relay.blocks[0].operations.clear();
    relay.blocks[0].terminator = Terminator::Conditional {
        condition: value_id(60),
        when_true: successor(5, 3),
        when_false: successor(6, 4),
    };
    relay.blocks.extend([second_arm, first_arm]);
    module
}

#[test]
fn branch_selected_record_return_keeps_one_permission_until_release() {
    verify_module(
        &branch_selected_record_return(),
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("both return origins feed one live mutable record permission");
    let mut released = branch_selected_record_return();
    released.machines[0].blocks[0].operations.swap(1, 2);
    rejects_custody(&released);
}

fn selected_reference_with_sibling_access() -> TerminalModule {
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, StructuralCaseId, StructuralFieldId,
    };
    use terminal_psi::{BindingRelevance, StructuralCaseDeclaration};

    let mut module = branch_selected_record_return();
    let status_type = structural_type_id(6);
    module.structural_types.push(StructuralTypeDeclaration {
        id: status_type,
        identity: "Status".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![StructuralCaseDeclaration {
                id: StructuralCaseId::new(1).unwrap(),
                identity: "Ready".into(),
                fields: Vec::new(),
            }],
        },
    });
    for owner in [structural_type_id(1), structural_type_id(5)] {
        let StructuralTypeShape::Record { fields } = &mut module
            .structural_types
            .iter_mut()
            .find(|row| row.id == owner)
            .unwrap()
            .shape
        else {
            unreachable!()
        };
        fields.push(StructuralFieldDeclaration {
            id: StructuralFieldId::new(4).unwrap(),
            identity: "status".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(status_type),
        });
    }
    let unrelated_element = vec![
        StructuralPathSegment::Field("holders".into()),
        StructuralPathSegment::FixedIndex(2),
    ];
    let operations = &mut module.machines[0].blocks[0].operations;
    operations.splice(
        1..1,
        [
            Operation {
                id: operation_id(70),
                static_reach_binding: None,
                suspension_crossing: None,
                result: OperationResult::Scalar(ValueDeclaration {
                    id: value_id(70),
                    scalar_type: ScalarType::Boolean,
                    qualifications: Default::default(),
                }),
                kind: OperationKind::StructuralCaseMembership {
                    source: place_id(1),
                    path: vec![StructuralPathSegment::Field("status".into())],
                    case: StructuralCaseId::new(1).unwrap(),
                },
            },
            Operation {
                id: operation_id(71),
                static_reach_binding: None,
                suspension_crossing: None,
                result: OperationResult::Scalar(ValueDeclaration {
                    id: value_id(71),
                    scalar_type: signed_i8(),
                    qualifications: Default::default(),
                }),
                kind: OperationKind::IntegerStructuralField {
                    source: place_id(1),
                    path: vec![
                        CanonicalStructuralPathSegment::Field(StructuralFieldId::new(3).unwrap()),
                        CanonicalStructuralPathSegment::FixedIndex(2),
                    ],
                    field: StructuralFieldId::new(1).unwrap(),
                },
            },
            Operation {
                id: operation_id(72),
                static_reach_binding: None,
                suspension_crossing: None,
                result: OperationResult::Unit,
                kind: OperationKind::StructuralScalarFieldStore {
                    destination: place_id(1),
                    path: unrelated_element,
                    field: StructuralFieldId::new(1).unwrap(),
                    value: value_id(1),
                    range_obligation: None,
                },
            },
        ],
    );
    module
}

#[test]
fn selected_reference_allows_disjoint_sibling_observations_and_stores() {
    verify_module(
        &selected_reference_with_sibling_access(),
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("root.status and holders[2] are disjoint from both possible selected holders");
}

#[test]
fn selected_reference_regions_reject_any_overlapping_candidate_and_whole_root() {
    use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralFieldId};

    let original = selected_reference_with_sibling_access();
    for selected_index in [0, 1] {
        for operation_index in [1, 2, 3] {
            let mut changed = original.clone();
            let operation = &mut changed.machines[0].blocks[0].operations[operation_index];
            let selected_path = vec![
                StructuralPathSegment::Field("holders".into()),
                StructuralPathSegment::FixedIndex(selected_index),
            ];
            match &mut operation.kind {
                OperationKind::StructuralCaseMembership { path, .. } => {
                    *path = selected_path;
                    path.push(StructuralPathSegment::Field("status".into()));
                }
                OperationKind::StructuralScalarFieldStore { path, .. } => *path = selected_path,
                OperationKind::IntegerStructuralField { path, .. } => {
                    *path = vec![
                        CanonicalStructuralPathSegment::Field(StructuralFieldId::new(3).unwrap()),
                        CanonicalStructuralPathSegment::FixedIndex(selected_index),
                    ];
                }
                _ => unreachable!(),
            }
            // Either one of the two candidates is enough to forbid access;
            // they describe alternative origins, not a chosen first origin.
            rejects_custody(&changed);
        }
    }

    let mut enclosing = original.clone();
    let caller = &mut enclosing.machines[0];
    let mut second_call = caller.blocks[0].operations[0].clone();
    second_call.id = operation_id(73);
    second_call.result = reference_result(6);
    caller.structural_places.push(reference_place(6, 73));
    caller.blocks[0].operations.insert(1, second_call);
    caller.blocks[0].operations.insert(2, release(74, 6));
    rejects_custody(&enclosing);

    let mut released = original;
    // Sibling access remains legal after release, but the existing write
    // through the returned carrier itself must still reject.
    let operations = &mut released.machines[0].blocks[0].operations;
    let release = operations.pop().unwrap();
    assert!(matches!(
        release.kind,
        OperationKind::ReleaseReference { .. }
    ));
    operations.insert(1, release);
    rejects_custody(&released);
}

#[test]
fn branch_selected_record_return_requires_exact_candidate_coverage() {
    validate_module(&branch_selected_record_return()).expect("honest candidate roster");
    for corruption in 0..7 {
        let mut module = branch_selected_record_return();
        let TerminalMachineResult::Structural(result) = &mut module.machines[1].result else {
            panic!("reference result");
        };
        let sources = &mut result.reference_sources[0].sources;
        match corruption {
            0 => {
                sources.remove(0);
            }
            1 => {
                let mut unused = sources[1].clone();
                unused.path[1] = StructuralPathSegment::FixedIndex(2);
                sources.push(unused);
            }
            2 => sources.insert(0, sources[0].clone()),
            3 => sources.reverse(),
            4 => sources[0].place = place_id(99),
            5 => sources[0].access = StructuralAccess::Owned,
            6 => sources[0].path[1] = StructuralPathSegment::FixedIndex(9),
            _ => unreachable!(),
        }
        assert!(
            validate_module(&module).is_err(),
            "candidate corruption {corruption}"
        );
    }
}

fn forwarded_selected_record_return() -> TerminalModule {
    let mut module = branch_selected_record_return();
    let mut forward = module.machines[1].clone();
    forward.id = machine_id(3);
    forward.entry = block_id(5);
    forward.contract = empty_contract(contract_id(3));
    forward.parameters[0].id = value_id(70);
    forward.structural_parameters[0].place = place_id(30);
    forward.structural_places = vec![
        StructuralPlaceDeclaration {
            id: place_id(30),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(31),
            kind: StructuralPlaceKind::Result,
        },
        reference_place(32, 30),
    ];
    let TerminalMachineResult::Structural(result) = &mut forward.result else {
        panic!("result");
    };
    result.place = place_id(31);
    for source in &mut result.reference_sources[0].sources {
        source.place = place_id(30);
    }
    forward.blocks.truncate(1);
    forward.blocks[0].id = block_id(5);
    forward.blocks[0].operations = vec![Operation {
        id: operation_id(30),
        static_reach_binding: None,
        suspension_crossing: None,
        result: reference_result(32),
        kind: OperationKind::CallStructuralWithScalarArguments {
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            callee: machine_id(2),
            arguments: vec![value_id(70)],
            structural_arguments: vec![argument(30, false)],
            claim_transfers: Vec::new(),
            returned_claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }];
    forward.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(10),
        source: place_id(32),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let OperationKind::CallStructuralWithScalarArguments { callee, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        panic!("call");
    };
    *callee = machine_id(3);
    module.machines.push(forward);
    module
}

#[test]
fn branch_selected_reference_forwarding_preserves_the_candidate_set() {
    verify_module(
        &forwarded_selected_record_return(),
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("ordinary forwarding preserves one selected carrier");
}

#[test]
fn branch_selected_reference_candidates_cannot_justify_themselves_recursively() {
    let mut module = forwarded_selected_record_return();
    let OperationKind::CallStructuralWithScalarArguments { callee, .. } =
        &mut module.machines[2].blocks[0].operations[0].kind
    else {
        panic!("call");
    };
    *callee = machine_id(3);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidReferenceCustody {
            machine: machine_id(3),
            reason: "recursive selected-reference results require founded origin summaries",
        })
    );
}

#[test]
fn branch_selected_reference_candidates_reject_recursive_singleton_dependencies() {
    let mut module = forwarded_selected_record_return();
    let mut recursive_dependency = module.machines[2].clone();
    recursive_dependency.id = machine_id(2);
    recursive_dependency.contract = empty_contract(contract_id(2));
    let TerminalMachineResult::Structural(result) = &mut recursive_dependency.result else {
        panic!("reference result");
    };
    result.reference_sources[0].sources.truncate(1);
    // The dependency declares one origin but only calls itself; its signature
    // cannot found an origin in the outer machine's alternative roster.
    module.machines[1] = recursive_dependency;
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidReferenceCustody {
            machine: machine_id(3),
            reason: "recursive selected-reference results require founded origin summaries",
        })
    );
}

fn projected_record_call(indexed: bool, access: StructuralAccess) -> TerminalModule {
    let mut module = returned_record_store(true, false);
    let mut writer = relay_module(false).machines.pop().expect("writer machine");
    let marker = semantic_vocabulary::StructuralFieldId::new(4).unwrap();
    module.structural_types.extend([
        StructuralTypeDeclaration {
            id: structural_type_id(6),
            identity: "Child".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: marker,
                    identity: "marker".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(signed_i8()),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(7),
            identity: "Children".into(),
            shape: StructuralTypeShape::FixedArray {
                element: structural_type_id(6),
                length: 2,
            },
        },
    ]);
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("holder record");
    };
    fields.extend([
        StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(5).unwrap(),
            identity: "child".into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(structural_type_id(6)),
        },
        StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(6).unwrap(),
            identity: "children".into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(structural_type_id(7)),
        },
    ]);
    writer.structural_parameters[0].structural_type = structural_type_id(6);
    writer.structural_parameters[0].access = access;
    writer.blocks[0].operations[0].kind = OperationKind::StructuralScalarFieldStore {
        destination: place_id(30),
        path: Vec::new(),
        field: marker,
        value: value_id(30),
        range_obligation: None,
    };
    if access == StructuralAccess::SharedBorrow {
        writer.blocks[0].operations[0].kind = OperationKind::IntegerStructuralField {
            source: place_id(30),
            path: Vec::new(),
            field: marker,
        };
        writer.blocks[0].operations[0].result = OperationResult::Scalar(ValueDeclaration {
            id: value_id(31),
            scalar_type: signed_i8(),
            qualifications: Default::default(),
        });
    }
    let mut path = vec![
        StructuralPathSegment::Referent,
        StructuralPathSegment::Field(if indexed { "children" } else { "child" }.into()),
    ];
    if indexed {
        path.push(StructuralPathSegment::FixedIndex(1));
    }
    module.machines[0].blocks[0].operations[1].kind = OperationKind::CallUnit {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        callee: machine_id(3),
        arguments: vec![value_id(1)],
        structural_arguments: vec![StructuralArgument {
            place: place_id(2),
            path,
            access,
        }],
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    module.machines.push(writer);
    module
}

#[test]
fn returned_record_projected_call_retains_path_and_access() {
    for indexed in [false, true] {
        for access in [
            StructuralAccess::MutableBorrow,
            StructuralAccess::SharedBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            verify_module(
                &projected_record_call(indexed, access),
                &ProofBundle::default(),
                &AdmissionProfile::default(),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "exact child call failed for indexed={indexed}, access={access:?}: {error:?}"
                )
            });
        }
    }
}

#[test]
fn returned_record_projected_call_rejects_path_access_and_lifetime_corruption() {
    validate_module(&projected_record_call(
        true,
        StructuralAccess::MutableBorrow,
    ))
    .expect("uncorrupted projected call is valid");
    for corruption in 0..9 {
        let mut module = projected_record_call(true, StructuralAccess::MutableBorrow);
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[1].kind
        else {
            panic!("writer call");
        };
        match corruption {
            0 => {
                structural_arguments[0].path.remove(0);
            }
            1 => structural_arguments[0]
                .path
                .insert(1, StructuralPathSegment::Referent),
            2 => structural_arguments[0].path[1] = "missing".into(),
            3 => structural_arguments[0].path[2] = StructuralPathSegment::FixedIndex(2),
            4 => structural_arguments[0].access = StructuralAccess::Owned,
            5 => structural_arguments[0].place = place_id(99),
            6 => module.machines[0].blocks[0].operations.swap(1, 2),
            7 => module.machines[0].blocks[0].operations.swap(0, 1),
            8 => {
                let StructuralTypeShape::Reference { access, .. } =
                    &mut module.structural_types[1].shape
                else {
                    panic!("reference");
                };
                *access = StructuralAccess::SharedBorrow;
            }
            _ => unreachable!(),
        }
        assert!(validate_module(&module).is_err(), "corruption {corruption}");
    }
}

#[test]
fn returned_record_projected_call_rejects_suspended_parent_and_overlapping_actuals() {
    let mut module = projected_record_call(false, StructuralAccess::MutableBorrow);
    validate_module(&module).expect("unsuspended projected call is valid");
    module.machines[0]
        .structural_places
        .push(reference_place(3, 9));
    module.machines[0].blocks[0]
        .operations
        .insert(1, establish(9, 3, argument(2, true)));
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidReferenceCustody {
            machine: machine_id(1),
            reason: "reference call descriptor is absent, ambiguous or suspended",
        })
    );

    let mut module = projected_record_call(false, StructuralAccess::MutableBorrow);
    let mut parameter = module.machines[2].structural_parameters[0].clone();
    parameter.place = place_id(31);
    parameter.position = 1;
    module.machines[2].structural_parameters.push(parameter);
    module.machines[2]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place_id(31),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        panic!("writer call");
    };
    structural_arguments.push(structural_arguments[0].clone());
    assert!(
        matches!(validate_module(&module), Err(ModuleError::OverlappingExclusiveStructuralArguments { operation, first_argument: 0, second_argument: 1 }) if operation == operation_id(2)),
        "{:?}",
        validate_module(&module)
    );
}

#[test]
fn returned_record_projected_call_does_not_admit_escaping_origins() {
    let mut module = projected_record_call(false, StructuralAccess::MutableBorrow);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(8),
        identity: "MutableChildReference".into(),
        shape: StructuralTypeShape::Reference {
            referent: structural_type_id(6),
            access: StructuralAccess::MutableBorrow,
        },
    });
    let writer = &mut module.machines[2];
    writer.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: place_id(32),
        structural_type: structural_type_id(8),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: vec![StructuralReferenceResultSource {
            path: Vec::new(),
            sources: vec![argument(30, false)],
        }],
    });
    writer.structural_places.extend([
        StructuralPlaceDeclaration {
            id: place_id(32),
            kind: StructuralPlaceKind::Result,
        },
        StructuralPlaceDeclaration {
            id: place_id(33),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(31),
                structural_type: structural_type_id(8),
            },
        },
    ]);
    let mut child = establish(31, 33, argument(30, false));
    let OperationResult::Structural(child_result) = &mut child.result else {
        panic!("reference result");
    };
    child_result.structural_type = structural_type_id(8);
    writer.blocks[0].operations.push(child);
    writer.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(3),
        source: place_id(33),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let caller = &mut module.machines[0];
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(3),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(2),
            structural_type: structural_type_id(8),
        },
    });
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &caller.blocks[0].operations[1].kind
    else {
        panic!("writer call");
    };
    caller.blocks[0].operations[1].kind = OperationKind::CallStructuralWithScalarArguments {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        callee: machine_id(3),
        arguments: vec![value_id(1)],
        structural_arguments: structural_arguments.clone(),
        claim_transfers: Vec::new(),
        returned_claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let mut result = reference_result(3);
    let OperationResult::Structural(call_result) = &mut result else {
        panic!("reference result");
    };
    call_result.structural_type = structural_type_id(8);
    caller.blocks[0].operations[1].result = result;
    caller.blocks[0].operations.insert(2, release(12, 3));
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidReferenceCustody {
            machine: machine_id(1),
            reason: "reference argument does not match its exact parameter",
        })
    );
}

#[test]
fn returned_record_field_store_rejects_changed_origin_access_and_lifetime() {
    for byte_store in [false, true] {
        let module = returned_record_store(true, byte_store);
        let store_position = if byte_store { 3 } else { 1 };
        for corruption in 0..7 {
            let mut changed = module.clone();
            match corruption {
                0 => {
                    changed.machines[0].blocks[0]
                        .operations
                        .swap(store_position, store_position + 1);
                }
                1 => {
                    let (OperationKind::StructuralScalarFieldStore { path, .. }
                    | OperationKind::StructuralByteSequenceFieldStore { path, .. }) =
                        &mut changed.machines[0].blocks[0].operations[store_position].kind
                    else {
                        unreachable!()
                    };
                    path.clear();
                }
                2 => {
                    let StructuralTypeShape::Reference { access, .. } =
                        &mut changed.structural_types[1].shape
                    else {
                        unreachable!()
                    };
                    *access = StructuralAccess::SharedBorrow;
                }
                3 => {
                    let TerminalMachineResult::Structural(result) = &mut changed.machines[1].result
                    else {
                        unreachable!()
                    };
                    result.reference_sources[0].sources[0].path[1] =
                        StructuralPathSegment::FixedIndex(0);
                }
                4 => {
                    let OperationKind::EstablishReference { source } =
                        &mut changed.machines[1].blocks[0].operations[0].kind
                    else {
                        unreachable!()
                    };
                    source.path[1] = StructuralPathSegment::FixedIndex(2);
                }
                5 => {
                    let (OperationKind::StructuralScalarFieldStore { destination, .. }
                    | OperationKind::StructuralByteSequenceFieldStore { destination, .. }) =
                        &mut changed.machines[0].blocks[0].operations[store_position].kind
                    else {
                        unreachable!()
                    };
                    *destination = place_id(99);
                }
                6 => {
                    changed.machines[0].blocks[0]
                        .operations
                        .swap(0, store_position);
                }
                _ => unreachable!(),
            }
            validate_module(&changed)
                .expect_err("reference field stores retain origin, access and lifetime");
        }
    }
}

#[test]
fn returned_record_field_store_requires_a_complete_uncalled_callee_roster() {
    let mut module = returned_record_store(true, false);
    let caller = &mut module.machines[0];
    caller.blocks[0].operations.clear();
    caller
        .structural_places
        .retain(|place| place.id == place_id(1));
    validate_module(&module).expect("the reference-returning callee need not have a caller");
    let TerminalMachineResult::Structural(result) = &mut module.machines[1].result else {
        unreachable!()
    };
    result.reference_sources.clear();
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidReferenceCustody {
            machine: machine_id(2),
            reason: "reference result is missing source mappings",
        })
    );
}

#[test]
fn returned_record_field_store_waits_for_child_release() {
    let mut module = returned_record_store(true, false);
    let caller = &mut module.machines[0];
    caller.structural_places.push(reference_place(3, 9));
    caller.blocks[0]
        .operations
        .splice(1..1, [establish(9, 3, argument(2, true)), release(12, 3)]);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a returned record may be written after its child loan ends");
    module.machines[0].blocks[0].operations.remove(2);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidReferenceCustody {
            machine: machine_id(1),
            reason: "field destination reference remains suspended by a live child",
        })
    );
}

#[test]
fn reference_result_replays_ordinary_relay_and_mark_before_return() {
    for mark in [false, true] {
        let module = relay_module(mark);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("reference custody is reconstructed without a supplied loan manifest");
    }
}

#[test]
fn reference_result_rejects_parent_read_write_and_call_before_release() {
    let module = relay_module(false);
    let mut read = module.clone();
    read.machines[0].blocks[0].operations.swap(2, 3);
    rejects_custody(&read);
    let mut write = module.clone();
    write.machines[0].blocks[0].operations.insert(
        2,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(5),
            result: OperationResult::Unit,
            kind: OperationKind::WriteOnlyPrimitiveStore {
                path: Vec::new(),
                destination: place_id(1),
                value: value_id(1),
            },
        },
    );
    rejects_custody(&write);
    let mut call = module;
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut call.machines[0].blocks[0].operations[1].kind
    else {
        panic!("writer");
    };
    structural_arguments[0] = argument(1, false);
    rejects_custody(&call);
}

#[test]
fn reference_result_rejects_duplicate_release_and_stale_carrier_use() {
    let mut duplicate = relay_module(false);
    duplicate.machines[0].blocks[0]
        .operations
        .insert(3, release(5, 2));
    rejects_custody(&duplicate);
    let mut stale = relay_module(false);
    stale.machines[0].blocks[0].operations.swap(1, 2);
    rejects_custody(&stale);
}

#[test]
fn reference_result_rejects_wrong_formal_and_local_escape() {
    let mut wrong = relay_module(false);
    let relay = &mut wrong.machines[1];
    let mut parameter = relay.structural_parameters[0].clone();
    parameter.place = place_id(13);
    parameter.position = 1;
    relay.structural_parameters.push(parameter);
    relay.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(13),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let TerminalMachineResult::Structural(result) = &mut relay.result else {
        panic!("reference result");
    };
    result.reference_sources[0].sources[0] = argument(13, false);
    // Validate the relay directly so unrelated caller arity is not the control.
    wrong.entry = machine_id(2);
    wrong.machines.remove(0);
    rejects_custody(&wrong);

    let mut escaped = relay_module(true);
    let relay = &mut escaped.machines[1];
    relay.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(13),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(19),
            structural_type: structural_type_id(1),
        },
    });
    relay.blocks[0].operations[1] = Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(19),
        result: OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: place_id(13),
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishPrimitiveLocal {
            value: value_id(10),
        },
    };
    relay.blocks[0].operations[2].kind = OperationKind::EstablishReference {
        source: argument(13, false),
    };
    rejects_custody(&escaped);
}

#[test]
fn reference_result_reborrow_suspends_immediate_parent_and_restores_deepest_first() {
    let mut module = relay_module(false);
    let caller = &mut module.machines[0];
    caller.structural_places.push(reference_place(3, 5));
    caller.blocks[0]
        .operations
        .splice(1..1, [establish(5, 3, argument(2, true)), release(6, 3)]);
    validate_module(&module).expect("ordinary nested reborrow can end before its parent is reused");
    let mut early_parent = module.clone();
    early_parent.machines[0].blocks[0].operations[2] = release(6, 2);
    rejects_custody(&early_parent);
    let mut parent_use = module;
    parent_use.machines[0].blocks[0].operations.swap(2, 3);
    rejects_custody(&parent_use);
}

#[test]
fn reference_result_second_carrier_cannot_duplicate_root_authority() {
    let mut module = relay_module(false);
    let caller = &mut module.machines[0];
    caller.structural_places.push(reference_place(3, 5));
    caller.blocks[0]
        .operations
        .insert(1, establish(5, 3, argument(1, false)));
    rejects_custody(&module);
}

#[test]
fn reference_result_interface_and_access_counterfeits_reject() {
    for change in 0..5 {
        let mut module = relay_module(false);
        let TerminalMachineResult::Structural(result) = &mut module.machines[1].result else {
            panic!("reference result");
        };
        match change {
            0 => result.reference_sources.clear(),
            1 => result
                .reference_sources
                .push(result.reference_sources[0].clone()),
            2 => result.reference_sources[0].sources[0].access = StructuralAccess::Owned,
            3 => result.reference_sources[0]
                .path
                .push(StructuralPathSegment::Referent),
            4 => result.multiplicity = StructuralMultiplicity::Unrestricted,
            _ => unreachable!(),
        }
        assert!(validate_module(&module).is_err());
    }
}

/// A shared `&i8` formal lends only shared. A reference minted from it and
/// returned may not claim mutable access, whether the widening is spelled
/// on the source argument or on the reference type.
#[test]
fn reference_formation_never_exceeds_a_shared_formal() {
    let shared_relay = |source_access, reference_access| {
        let mut module = relay_module(false);
        for declaration in &mut module.structural_types {
            if let StructuralTypeShape::Reference { access, .. } = &mut declaration.shape {
                *access = reference_access;
            }
        }
        let relay = &mut module.machines[1];
        relay.structural_parameters[0].access = StructuralAccess::SharedBorrow;
        let TerminalMachineResult::Structural(result) = &mut relay.result else {
            panic!("reference result");
        };
        result.reference_sources[0].sources[0].access = source_access;
        let OperationKind::EstablishReference { source } = &mut relay.blocks[0].operations[0].kind
        else {
            panic!("establishment");
        };
        source.access = source_access;
        // Validate the relay directly so caller-side access is not the control.
        module.entry = machine_id(2);
        module.machines.remove(0);
        module
    };
    rejects_custody(&shared_relay(
        StructuralAccess::MutableBorrow,
        StructuralAccess::MutableBorrow,
    ));
    rejects_custody(&shared_relay(
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ));
}

fn jump(edge: u64, target: u64, discards: Vec<PlaceId>) -> Terminator {
    Terminator::Jump {
        edge: edge_id(edge),
        target: block_id(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: discards,
        residual_affine_discards: Vec::new(),
    }
}

#[test]
fn reference_result_edge_discard_releases_permission_before_continuing_read() {
    let mut module = relay_module(false);
    let caller = &mut module.machines[0];
    let read = caller.blocks[0].operations.pop().expect("final read");
    caller.blocks[0].operations.pop().expect("explicit release");
    let completion = caller.blocks[0].terminator.clone();
    caller.blocks[0].terminator = jump(4, 4, vec![place_id(2)]);
    caller.blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        id: block_id(4),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: vec![read],
        terminator: completion,
    });
    validate_module(&module).expect("edge no-code disposal releases the loan, never the referent");
    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        panic!("jump");
    };
    trivial_affine_discards.clear();
    rejects_custody(&module);
}

#[test]
fn reference_result_join_requires_equal_reference_custody_on_every_arrival() {
    let mut module = relay_module(false);
    let caller = &mut module.machines[0];
    let read = caller.blocks[0].operations.pop().expect("final read");
    let release = caller.blocks[0].operations.pop().expect("release");
    let completion = caller.blocks[0].terminator.clone();
    caller.blocks[0].operations.push(Operation {
        id: operation_id(5),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Scalar(ValueDeclaration {
            id: value_id(5),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }),
        kind: OperationKind::BooleanConstant { value: true },
    });
    let successor = |edge, target| SuccessorEdge {
        edge: edge_id(edge),
        target: block_id(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    caller.blocks[0].terminator = Terminator::Conditional {
        condition: value_id(5),
        when_true: successor(4, 4),
        when_false: successor(5, 5),
    };
    let mut other_release = release.clone();
    other_release.id = operation_id(6);
    caller.blocks.extend([
        Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: block_id(4),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![release],
            terminator: jump(6, 6, Vec::new()),
        },
        Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: block_id(5),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![other_release],
            terminator: jump(7, 6, Vec::new()),
        },
        Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: block_id(6),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![read],
            terminator: completion,
        },
    ]);
    validate_module(&module).expect("both branch-local releases restore the same original root");
    module.machines[0].blocks[2].operations.clear();
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::OwnedStructuralFrontierJoinMismatch(block_id(6))
    );
}

#[test]
fn reference_result_alias_writes_invalidate_reaching_store_but_permission_events_do_not() {
    let mut module = relay_module(false);
    let caller = &mut module.machines[0];
    // Isolate the reference lifetime from the otherwise conservative effects
    // of a result-producing call; the later writer still uses the reference.
    caller.blocks[0].operations[0] = establish(1, 2, argument(1, false));
    caller.blocks[0].operations.insert(
        0,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(7),
            result: OperationResult::Unit,
            kind: OperationKind::WriteOnlyPrimitiveStore {
                path: Vec::new(),
                destination: place_id(1),
                value: value_id(1),
            },
        },
    );
    let term = |value| ScalarTerm::value(value_id(value), signed_i8());
    caller.contract.ensures.push(ContractClause {
        obligation: obligation_id(1),
        proposition: Proposition::Equal(term(99), term(1)),
    });
    let snapshot = Proposition::Equal(term(2), term(1));
    let has_snapshot = |module: &TerminalModule| {
        terminal_verifier::reconstruct_terminal_obligations(module)
            .expect("valid reference module with a return obligation")
            .obligations()
            .iter()
            .any(|obligation| obligation.semantic_axioms.contains(&snapshot))
    };
    assert!(
        !has_snapshot(&module),
        "a distinct carrier ID must not hide the writer's alias of the stored root"
    );
    module.machines[0].blocks[0].operations.remove(2);
    assert!(
        has_snapshot(&module),
        "forming and releasing permission does not modify the referent"
    );
}

#[test]
fn reference_result_mutable_carrier_lends_only_the_callees_declared_access() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = relay_module(false);
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[1].kind
        else {
            panic!("ordinary primitive call");
        };
        structural_arguments[0].access = access;
        let callee = &mut module.machines[2];
        callee.structural_parameters[0].access = access;
        if access == StructuralAccess::SharedBorrow {
            callee.blocks[0].operations[0] = Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation_id(30),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: value_id(31),
                    scalar_type: signed_i8(),
                    qualifications: Default::default(),
                }),
                kind: OperationKind::PrimitiveScalarRead {
                    path: Vec::new(),
                    source: place_id(30),
                },
            };
        }
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("call-local attenuation preserves mutable carrier custody and original backing");
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[1].kind
        else {
            panic!("ordinary primitive call");
        };
        structural_arguments[0].access = StructuralAccess::Owned;
        assert!(
            validate_module(&module).is_err(),
            "dereference cannot confer owned access"
        );
    }
}

#[test]
fn reference_result_host_projection_rejects_without_widening_boundary_support() {
    let mut module = relay_module(false);
    let mut parameter = module.machines[2].structural_parameters[0].clone();
    parameter.place = place_id(40);
    module.boundary_machines.push(BoundaryMachineDeclaration {
        unconditional_termination: false,
        completion: terminal_psi::BoundaryCompletion::NormalReturn,
        kind: terminal_psi::BoundaryRequirementKind::Trait,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
        id: boundary_id(1),
        identity: "primitive_boundary".into(),
        attachment: None,
        parameter_order: vec![
            terminal_psi::BoundaryParameterKind::Scalar,
            terminal_psi::BoundaryParameterKind::Structural,
        ],
        scalar_parameters: vec![signed_i8()],
        structural_parameters: vec![parameter],
        result: terminal_psi::BoundaryMachineResult::Unit,
        crash_routes: Vec::new(),
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    let operations = &mut module.machines[0].blocks[0].operations;
    operations[1].kind = OperationKind::BoundaryCall {
        requirement_obligations: Vec::new(),
        boundary: boundary_id(1),
        arguments: vec![value_id(1)],
        structural_arguments: vec![argument(1, false)],
        completion_receipts: Vec::new(),
    };
    operations.swap(1, 2);
    validate_module(&module)
        .expect("ordinary primitive boundary argument after restored use remains supported");
    let operations = &mut module.machines[0].blocks[0].operations;
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut operations[2].kind
    else {
        panic!("boundary call");
    };
    structural_arguments[0] = argument(2, true);
    operations.swap(1, 2);
    rejects_custody(&module);
}

fn stored_reference_argument() -> StructuralArgument {
    StructuralArgument {
        place: place_id(3),
        path: vec!["body".into(), StructuralPathSegment::Referent],
        access: StructuralAccess::MutableBorrow,
    }
}

fn record_reference_module() -> TerminalModule {
    let mut module = relay_module(false);
    let field = semantic_vocabulary::StructuralFieldId::new(1).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "View".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: field,
                identity: "body".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(structural_type_id(2)),
            }],
        },
    });
    let caller = &mut module.machines[0];
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(3),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(5),
            structural_type: structural_type_id(3),
        },
    });
    caller.blocks[0].operations[0] = establish(1, 2, argument(1, false));
    let read = caller.blocks[0].operations.pop().unwrap();
    caller.blocks[0].operations.pop().unwrap();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].operations[1].kind
    else {
        panic!("writer call");
    };
    structural_arguments[0] = stored_reference_argument();
    caller.blocks[0].operations.insert(
        1,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(5),
            result: OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: place_id(3),
                structural_type: structural_type_id(3),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishRecord {
                fields: vec![terminal_psi::RecordFieldInitializer {
                    field,
                    value: terminal_psi::RecordFieldValue::Structural(StructuralArgument {
                        place: place_id(2),
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    }),
                }],
            },
        },
    );
    let completion = caller.blocks[0].terminator.clone();
    caller.blocks[0].terminator = jump(4, 4, vec![place_id(3)]);
    caller.blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        id: block_id(4),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: vec![read],
        terminator: completion,
    });
    module
}

#[test]
fn reference_record_rejects_duplicate_child_and_unrestricted_owner() {
    let mut module = record_reference_module();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[2].shape else {
        panic!("record");
    };
    let mut second = fields[0].clone();
    second.id = semantic_vocabulary::StructuralFieldId::new(2).unwrap();
    second.identity = "other".into();
    let second_id = second.id;
    fields.push(second);
    let OperationKind::EstablishRecord { fields } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        panic!("constructor");
    };
    let mut duplicate = fields[0].clone();
    duplicate.field = second_id;
    fields.push(duplicate);
    rejects_custody(&module);

    let mut module = record_reference_module();
    let OperationResult::Structural(result) =
        &mut module.machines[0].blocks[0].operations[1].result
    else {
        panic!("record result");
    };
    result.multiplicity = StructuralMultiplicity::Unrestricted;
    assert!(
        validate_module(&module).is_err(),
        "an affine leaf cannot become copyable through its record"
    );
}

#[test]
fn reference_record_rejects_stale_carrier_parent_access_and_untyped_projection() {
    for change in 0..5 {
        let mut module = record_reference_module();
        let caller = &mut module.machines[0];
        match change {
            0 => caller.blocks[0].operations.insert(2, release(6, 2)),
            1 => {
                let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &mut caller.blocks[0].operations[2].kind
                else {
                    panic!("call");
                };
                structural_arguments[0] = argument(2, true);
            }
            2 => {
                let read = caller.blocks[1].operations.remove(0);
                caller.blocks[0].operations.push(read);
            }
            3 => {
                let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &mut caller.blocks[0].operations[2].kind
                else {
                    panic!("call");
                };
                structural_arguments[0].path[0] = "counterfeit".into();
            }
            4 => caller.blocks[0].operations.insert(2, release(6, 3)),
            _ => unreachable!(),
        }
        if change == 3 {
            assert!(
                matches!(validate_module(&module), Err(ModuleError::InvalidStructuralArgumentPath { operation, argument_index: 0 }) if operation == operation_id(2))
            );
        } else {
            rejects_custody(&module);
        }
    }
}

#[test]
fn reference_record_relocation_preserves_child_suspension_and_cleanup_order() {
    let mut module = record_reference_module();
    let caller = &mut module.machines[0];
    caller.structural_places.push(reference_place(4, 6));
    // Create the child before moving its parent. The parent's stable identity
    // must still suspend the new record leaf until the child is released.
    caller.blocks[0]
        .operations
        .insert(1, establish(6, 4, argument(2, true)));
    caller.blocks[0].operations.insert(3, release(7, 4));
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("owned relocation preserves the immediate parent relationship");
    let mut parent_use = module.clone();
    parent_use.machines[0].blocks[0].operations.swap(3, 4);
    rejects_custody(&parent_use);

    // Child survives across the record's discard edge and ends in the next
    // block. The edge must reject before the original referent can be restored.
    let caller = &mut module.machines[0];
    let child_release = caller.blocks[0].operations.remove(3);
    caller.blocks[0]
        .operations
        .pop()
        .expect("remove suspended-parent writer");
    caller.blocks[1].operations.insert(0, child_release);
    rejects_custody(&module);
}

#[test]
fn reference_record_rejects_unreplayed_producer_and_interface_custody() {
    for change in 0..4 {
        let mut module = record_reference_module();
        match change {
            0 => {
                module.machines[0].blocks[0].operations[1].kind =
                    OperationKind::EstablishReference {
                        source: argument(1, false),
                    }
            }
            1 => {
                module.machines[0].structural_parameters[0].structural_type = structural_type_id(3);
                module.machines[0].structural_parameters[0].access = StructuralAccess::Owned;
            }
            2 => {
                let TerminalMachineResult::Structural(result) = &mut module.machines[1].result
                else {
                    panic!("relay result");
                };
                result.structural_type = structural_type_id(3);
            }
            3 => {
                let mut parameter = module.machines[0].structural_parameters[0].clone();
                parameter.place = place_id(40);
                parameter.structural_type = structural_type_id(3);
                module.boundary_machines.push(BoundaryMachineDeclaration {
                    unconditional_termination: false,
                    completion: terminal_psi::BoundaryCompletion::NormalReturn,
                    kind: terminal_psi::BoundaryRequirementKind::Trait,
                    scalar_ensures: Vec::new(),
                    scalar_requires: Vec::new(),
                    id: boundary_id(1),
                    identity: "stored_reference_boundary".into(),
                    attachment: None,
                    parameter_order: vec![terminal_psi::BoundaryParameterKind::Structural],
                    scalar_parameters: Vec::new(),
                    structural_parameters: vec![parameter],
                    result: terminal_psi::BoundaryMachineResult::Unit,
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    program_local_root_introductions: Vec::new(),
                    content_guarantees: Vec::new(),
                    fixed_service_reach: Vec::new(),
                    published_service_ceiling: Vec::new(),
                });
            }
            _ => unreachable!(),
        }
        assert!(
            validate_module(&module).is_err(),
            "unsupported custody boundary {change}"
        );
    }
}

fn returned_record_reference_module() -> TerminalModule {
    let mut module = record_reference_module();
    let caller = &mut module.machines[0];
    let mut construction = caller.blocks[0].operations.remove(1);
    caller.blocks[0].operations.remove(0);
    caller
        .structural_places
        .retain(|place| place.id != place_id(2));
    caller.blocks[0].operations.insert(
        0,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(5),
            result: construction.result.clone(),
            kind: OperationKind::CallStructural {
                callee: machine_id(2),
                structural_arguments: vec![argument(1, false)],
                claim_transfers: Vec::new(),
                returned_claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                selected_evidence: Vec::new(),
            },
        },
    );
    construction.id = operation_id(21);
    let OperationResult::Structural(result) = &mut construction.result else {
        panic!("record");
    };
    result.place = place_id(13);
    let OperationKind::EstablishRecord { fields } = &mut construction.kind else {
        panic!("constructor");
    };
    let terminal_psi::RecordFieldValue::Structural(source) = &mut fields[0].value else {
        panic!("reference field");
    };
    source.place = place_id(12);
    let callee = &mut module.machines[1];
    callee.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(13),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(21),
            structural_type: structural_type_id(3),
        },
    });
    let TerminalMachineResult::Structural(result) = &mut callee.result else {
        panic!("result signature");
    };
    result.structural_type = structural_type_id(3);
    result.reference_sources[0].path = vec!["body".into()];
    callee.blocks[0].operations.push(construction);
    let Terminator::ReturnStructural { source, .. } = &mut callee.blocks[0].terminator else {
        panic!("return");
    };
    *source = place_id(13);
    module
}

pub(super) fn owned_reference_record_module(two_leaves: bool, consumes: bool) -> TerminalModule {
    let mut module = if two_leaves {
        two_reference_result_module()
    } else {
        returned_record_reference_module()
    };
    let mut forward = module.machines[1].clone();
    forward.id = machine_id(4);
    forward.entry = block_id(5);
    forward.contract = empty_contract(contract_id(4));
    forward.structural_parameters.truncate(1);
    let parameter = &mut forward.structural_parameters[0];
    parameter.place = place_id(40);
    parameter.structural_type = structural_type_id(3);
    parameter.access = StructuralAccess::Owned;
    parameter.multiplicity = StructuralMultiplicity::Affine;
    forward.structural_places = vec![
        StructuralPlaceDeclaration {
            id: place_id(40),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(41),
            kind: StructuralPlaceKind::Result,
        },
    ];
    let TerminalMachineResult::Structural(result) = &mut forward.result else {
        panic!("record result")
    };
    result.place = place_id(41);
    for mapping in &mut result.reference_sources {
        mapping.sources[0] = StructuralArgument {
            place: place_id(40),
            path: mapping
                .path
                .iter()
                .cloned()
                .chain([StructuralPathSegment::Referent])
                .collect(),
            access: StructuralAccess::MutableBorrow,
        };
    }
    forward.blocks[0].id = block_id(5);
    forward.blocks[0].operations.clear();
    forward.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(5),
        source: place_id(40),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let caller = &mut module.machines[0];
    let mut call = caller.blocks[0].operations[0].clone();
    call.id = operation_id(6);
    let OperationResult::Structural(result) = &mut call.result else {
        panic!("record call")
    };
    result.place = place_id(6);
    let owned = StructuralArgument {
        place: place_id(3),
        path: Vec::new(),
        access: StructuralAccess::Owned,
    };
    call.kind = OperationKind::CallStructural {
        callee: machine_id(4),
        structural_arguments: vec![owned.clone()],
        claim_transfers: Vec::new(),
        returned_claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        selected_evidence: Vec::new(),
    };
    if consumes {
        forward.result = TerminalMachineResult::Unit;
        forward.structural_places.pop();
        forward.blocks[0].terminator = Terminator::ReturnUnit {
            edge: edge_id(5),
            trivial_affine_discards: vec![place_id(40)],
        };
        call.result = OperationResult::Unit;
        call.kind = OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            callee: machine_id(4),
            arguments: Vec::new(),
            structural_arguments: vec![owned],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        };
        caller.blocks[0].operations.pop();
        caller.blocks[0].terminator = jump(4, 4, Vec::new());
    } else {
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(6),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(6),
                structural_type: structural_type_id(3),
            },
        });
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut caller.blocks[0].operations[1].kind
        else {
            panic!("writer")
        };
        structural_arguments[0].place = place_id(6);
        caller.blocks[0].terminator = jump(4, 4, vec![place_id(6)]);
    }
    caller.blocks[0].operations.insert(1, call);
    module.machines.push(forward);
    module
}

#[test]
fn owned_reference_record_ingress_forwards_or_disposes_existing_loans() {
    for two_leaves in [false, true] {
        for consumes in [false, true] {
            verify_module(
                &owned_reference_record_module(two_leaves, consumes),
                &ProofBundle::default(),
                &AdmissionProfile::default(),
            )
            .expect("owned ingress transfers existing leaf permissions without forming children");
        }
    }
}

#[test]
fn owned_reference_record_rejects_changed_maps_stale_owners_and_missing_disposal() {
    for change in 0..6 {
        let mut module = owned_reference_record_module(true, change == 5);
        match change {
            0..=3 => {
                let TerminalMachineResult::Structural(result) = &mut module.machines[3].result
                else {
                    panic!("forward result")
                };
                match change {
                    0 => {
                        result.reference_sources.pop();
                    }
                    1 => {
                        result.reference_sources[1].sources[0] =
                            result.reference_sources[0].sources[0].clone();
                    }
                    2 => {
                        let source = result.reference_sources[0].sources[0].clone();
                        result.reference_sources[0].sources[0] =
                            result.reference_sources[1].sources[0].clone();
                        result.reference_sources[1].sources[0] = source;
                    }
                    3 => {
                        result.reference_sources[0].sources[0].path.pop();
                    }
                    _ => unreachable!(),
                }
            }
            4 => {
                let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &mut module.machines[0].blocks[0].operations[2].kind
                else {
                    panic!("writer")
                };
                structural_arguments[0].place = place_id(3);
            }
            5 => {
                let Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } = &mut module.machines[3].blocks[0].terminator
                else {
                    panic!("consume")
                };
                trivial_affine_discards.clear();
            }
            _ => unreachable!(),
        }
        assert!(
            validate_module(&module).is_err(),
            "changed owned ingress custody {change}"
        );
    }
}

#[test]
fn owned_reference_record_rejects_live_descendant_and_aliased_borrowed_argument() {
    for descendant in [false, true] {
        let mut module = owned_reference_record_module(false, false);
        if descendant {
            let caller = &mut module.machines[0];
            caller.structural_places.push(reference_place(7, 7));
            caller.blocks[0]
                .operations
                .insert(1, establish(7, 7, stored_reference_argument()));
            caller.blocks[1].operations.insert(0, release(8, 7));
        } else {
            let mut parameter = module.machines[0].structural_parameters[0].clone();
            parameter.position = 1;
            parameter.place = place_id(42);
            let forward = &mut module.machines[3];
            forward.structural_parameters.push(parameter);
            forward.structural_places.push(StructuralPlaceDeclaration {
                id: place_id(42),
                kind: StructuralPlaceKind::Parameter {
                    position: 1,
                    is_self: false,
                },
            });
            let OperationKind::CallStructural {
                structural_arguments,
                ..
            } = &mut module.machines[0].blocks[0].operations[1].kind
            else {
                panic!("forward")
            };
            structural_arguments.push(argument(1, false));
        }
        rejects_custody(&module);
    }
}

#[test]
fn owned_reference_record_ingress_preserves_external_parent_until_returned_child_ends() {
    let mut module = owned_reference_record_module(false, false);
    let caller = &mut module.machines[0];
    caller.structural_places.push(reference_place(7, 7));
    let OperationKind::CallStructural {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].operations[0].kind
    else {
        panic!("make view")
    };
    structural_arguments[0] = argument(7, true);
    caller.blocks[0]
        .operations
        .insert(0, establish(7, 7, argument(1, false)));
    caller.blocks[1].operations.insert(0, release(8, 7));
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("forward preserves the child's external parent identity through owned transfer");
    module.machines[0].blocks[0]
        .operations
        .insert(3, release(9, 7));
    rejects_custody(&module);
}

#[test]
fn owned_reference_record_ingress_bounds_type_dag_expansion() {
    let mut module = owned_reference_record_module(false, true);
    let forward = module.machines.pop().unwrap();
    module.machines = vec![forward];
    let mut child = structural_type_id(3);
    for level in 0..13_u64 {
        let current = structural_type_id(100 + level);
        module.structural_types.push(StructuralTypeDeclaration {
            id: current,
            identity: format!("IncomingBranch{level}"),
            shape: StructuralTypeShape::Record {
                fields: (0..2_u64)
                    .map(|field| StructuralFieldDeclaration {
                        id: semantic_vocabulary::StructuralFieldId::new(100 + level * 2 + field)
                            .unwrap(),
                        identity: format!("branch{field}"),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(child),
                    })
                    .collect(),
            },
        });
        child = current;
    }
    module.machines[0].structural_parameters[0].structural_type = child;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::InvalidReferenceCustody {
            reason: "incoming reference leaf reconstruction capacity exceeded",
            ..
        })
    ));
}

fn two_reference_result_module() -> TerminalModule {
    let mut module = returned_record_reference_module();
    let second_field = semantic_vocabulary::StructuralFieldId::new(2).unwrap();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[2].shape else {
        panic!("record");
    };
    let mut field = fields[0].clone();
    field.id = second_field;
    field.identity = "other".into();
    fields.push(field);

    let callee = &mut module.machines[1];
    let mut parameter = callee.structural_parameters[0].clone();
    parameter.place = place_id(14);
    parameter.position = 1;
    callee.structural_parameters.push(parameter);
    callee.structural_places.extend([
        StructuralPlaceDeclaration {
            id: place_id(14),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
        reference_place(15, 22),
    ]);
    callee.blocks[0]
        .operations
        .insert(1, establish(22, 15, argument(14, false)));
    let OperationKind::EstablishRecord { fields } = &mut callee.blocks[0].operations[2].kind else {
        panic!("constructor");
    };
    fields.push(terminal_psi::RecordFieldInitializer {
        field: second_field,
        value: terminal_psi::RecordFieldValue::Structural(StructuralArgument {
            place: place_id(15),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }),
    });
    let TerminalMachineResult::Structural(result) = &mut callee.result else {
        panic!("result");
    };
    result
        .reference_sources
        .push(StructuralReferenceResultSource {
            path: vec!["other".into()],
            sources: vec![argument(14, false)],
        });

    let caller = &mut module.machines[0];
    let mut parameter = caller.structural_parameters[0].clone();
    parameter.place = place_id(4);
    parameter.position = 1;
    caller.structural_parameters.push(parameter);
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(4),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let OperationKind::CallStructural {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].operations[0].kind
    else {
        panic!("call");
    };
    structural_arguments.push(argument(4, false));
    module
}

#[test]
fn reference_record_result_sibling_loans_keep_distinct_formation_identities() {
    let mut module = two_reference_result_module();
    let caller = &mut module.machines[0];
    caller.structural_places.push(reference_place(5, 6));
    caller.blocks[0]
        .operations
        .insert(1, establish(6, 5, stored_reference_argument()));
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].operations[2].kind
    else {
        panic!("writer");
    };
    structural_arguments[0].path[0] = "other".into();
    caller.blocks[0].operations.push(release(7, 5));
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a child of body suspends body, not the sibling returned by the same operation");
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[2].kind
    else {
        panic!("writer");
    };
    structural_arguments[0].path[0] = "body".into();
    rejects_custody(&module);
}

#[test]
fn reference_record_result_map_order_is_independent_of_declaration_order() {
    let mut module = two_reference_result_module();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[2].shape else {
        panic!("record");
    };
    fields.swap(0, 1);
    let OperationKind::EstablishRecord { fields } =
        &mut module.machines[1].blocks[0].operations[2].kind
    else {
        panic!("constructor");
    };
    fields.swap(0, 1);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("declarations and construction use other/body; the source map stays body/other");
    let TerminalMachineResult::Structural(result) = &mut module.machines[1].result else {
        panic!("result");
    };
    result.reference_sources.swap(0, 1);
    rejects_custody(&module);
}

#[test]
fn reference_record_result_requires_exact_ordered_source_roster() {
    for change in 0..9 {
        let mut module = two_reference_result_module();
        let TerminalMachineResult::Structural(result) = &mut module.machines[1].result else {
            panic!("result");
        };
        match change {
            0 => {
                result.reference_sources.pop();
            }
            1 => result
                .reference_sources
                .push(result.reference_sources[0].clone()),
            2 => result.reference_sources.swap(0, 1),
            3 => {
                result.reference_sources[0].sources[0].place = place_id(14);
                result.reference_sources[1].sources[0].place = place_id(10);
            }
            4 => result.reference_sources[0].sources[0] = argument(14, false),
            5 => result.reference_sources[0].sources[0].access = StructuralAccess::SharedBorrow,
            6 => result.reference_sources[0].path = vec!["counterfeit".into()],
            7 => {
                module.structural_types[1].shape = StructuralTypeShape::Reference {
                    referent: structural_type_id(4),
                    access: StructuralAccess::MutableBorrow,
                };
                module.structural_types.push(StructuralTypeDeclaration {
                    id: structural_type_id(4),
                    identity: "BooleanReferent".into(),
                    shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                });
            }
            8 => result.reference_sources[0].sources[0].path = vec!["body".into()],
            _ => unreachable!(),
        }
        rejects_custody(&module);
    }
}

#[test]
fn reference_record_result_reborrows_stored_parent_without_early_restoration() {
    let mut module = returned_record_reference_module();
    let caller = &mut module.machines[0];
    let mut child = caller.blocks[0].operations[0].clone();
    child.id = operation_id(6);
    let OperationResult::Structural(result) = &mut child.result else {
        panic!("child result");
    };
    result.place = place_id(4);
    let OperationKind::CallStructural {
        structural_arguments,
        ..
    } = &mut child.kind
    else {
        panic!("child call");
    };
    structural_arguments[0] = stored_reference_argument();
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(4),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(6),
            structural_type: structural_type_id(3),
        },
    });
    caller.blocks[0].operations.insert(1, child);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].operations[2].kind
    else {
        panic!("writer");
    };
    structural_arguments[0].place = place_id(4);
    caller.blocks[0].terminator = jump(4, 4, vec![place_id(4), place_id(3)]);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("make_view(held.body) returns a child loan and retains the parent until child cleanup");
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[2].kind
    else {
        panic!("writer");
    };
    structural_arguments[0].place = place_id(3);
    rejects_custody(&module);
}

#[test]
fn reference_record_result_preserves_scalar_sibling_and_rejects_local_escape() {
    let mut module = returned_record_reference_module();
    let scalar_field = semantic_vocabulary::StructuralFieldId::new(2).unwrap();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[2].shape else {
        panic!("record");
    };
    fields.push(StructuralFieldDeclaration {
        id: scalar_field,
        identity: "mark".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(signed_i8()),
    });
    let callee = &mut module.machines[1];
    callee.blocks[0].operations.insert(
        0,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(18),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(10),
                scalar_type: signed_i8(),
                qualifications: Default::default(),
            }),
            kind: OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Signed(7),
            },
        },
    );
    let OperationKind::EstablishRecord { fields } = &mut callee.blocks[0].operations[2].kind else {
        panic!("record constructor");
    };
    fields.push(terminal_psi::RecordFieldInitializer {
        field: scalar_field,
        value: terminal_psi::RecordFieldValue::Scalar {
            value: value_id(10),
            range_obligation: None,
        },
    });
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("reference roster omits scalar payloads without rejecting ordinary record fields");

    let callee = &mut module.machines[1];
    callee.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(16),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(19),
            structural_type: structural_type_id(1),
        },
    });
    callee.blocks[0].operations.insert(
        1,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(19),
            result: OperationResult::Structural(StructuralOperationResult {
                place: place_id(16),
                structural_type: structural_type_id(1),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishPrimitiveLocal {
                value: value_id(10),
            },
        },
    );
    callee.blocks[0].operations[2].kind = OperationKind::EstablishReference {
        source: argument(16, false),
    };
    rejects_custody(&module);
}

fn scalar_reference_view_module(source: ScalarType, target: ScalarType) -> TerminalModule {
    let mut module = write_only_primitive_store_module();
    module.structural_types[0].shape = StructuralTypeShape::PrimitiveScalar(source);
    module.structural_types.extend([
        StructuralTypeDeclaration {
            id: structural_type_id(2),
            identity: "SharedScalarRepresentation".into(),
            shape: StructuralTypeShape::Reference {
                referent: structural_type_id(3),
                access: StructuralAccess::SharedBorrow,
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(3),
            identity: "ObservedScalar".into(),
            shape: StructuralTypeShape::PrimitiveScalar(target),
        },
    ]);
    let machine = &mut module.machines[0];
    machine.parameters[0].scalar_type = source;
    machine.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    machine.structural_places.push(reference_place(2, 1));
    machine.blocks[0].operations = vec![
        establish(
            1,
            2,
            StructuralArgument {
                place: place_id(1),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            },
        ),
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(2),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(2),
                scalar_type: target,
            }),
            kind: OperationKind::CallStructuralScalar {
                callee: machine_id(2),
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: place_id(2),
                    path: vec![StructuralPathSegment::Referent],
                    access: StructuralAccess::SharedBorrow,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        },
        release(3, 2),
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(4),
            result: OperationResult::Unit,
            kind: OperationKind::WriteOnlyPrimitiveStore {
                destination: place_id(1),
                path: Vec::new(),
                value: value_id(1),
            },
        },
    ];
    // The reference supplies a helper's ordinary primitive referent formal.
    // PrimitiveScalarRead observes that formal with an empty path; formation
    // and call custody authenticate the original backing independently.
    let mut reader = machine.clone();
    reader.id = machine_id(2);
    reader.entry = block_id(2);
    reader.contract = empty_contract(contract_id(2));
    reader.parameters.clear();
    reader.structural_parameters[0].place = place_id(10);
    reader.structural_parameters[0].structural_type = structural_type_id(3);
    reader.structural_parameters[0].access = StructuralAccess::SharedBorrow;
    reader.structural_places = vec![StructuralPlaceDeclaration {
        id: place_id(10),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    reader.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(12),
        scalar_type: target,
    });
    reader.blocks[0].id = block_id(2);
    reader.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(10),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(11),
            scalar_type: target,
        }),
        kind: OperationKind::PrimitiveScalarRead {
            source: place_id(10),
            path: Vec::new(),
        },
    }];
    reader.blocks[0].terminator = Terminator::Return {
        edge: edge_id(2),
        value: value_id(11),
        cleanup_actions: Vec::new(),
    };
    module.machines.push(reader);
    module
}

#[test]
fn shared_scalar_reference_views_preserve_backing_and_release_authority() {
    use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, IntegerType};
    let unsigned =
        |bits| ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap());
    for (source, target) in [
        (
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
            unsigned(32),
        ),
        (
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
            unsigned(64),
        ),
        (
            unsigned(32),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        ),
        (ScalarType::Boolean, unsigned(8)),
        (signed_i8(), unsigned(8)),
    ] {
        verify_module(
            &scalar_reference_view_module(source, target),
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("a total shared representation keeps its source home and releases its loan");
    }
}

#[test]
fn shared_scalar_reference_views_reject_invalid_formation_and_live_loan_use() {
    use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, IntegerType};
    let unsigned =
        |bits| ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap());
    let f32_type = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
    for (source, target) in [(f32_type, unsigned(64)), (unsigned(8), ScalarType::Boolean)] {
        rejects_custody(&scalar_reference_view_module(source, target));
    }
    let original = scalar_reference_view_module(f32_type, unsigned(32));
    verify_module(
        &original,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the helper-call reference fixture verifies before tampering");
    for mutation in 0..5 {
        let mut module = original.clone();
        match mutation {
            0 => {
                let StructuralTypeShape::Reference { access, .. } =
                    &mut module.structural_types[1].shape
                else {
                    unreachable!()
                };
                *access = StructuralAccess::MutableBorrow;
            }
            1 => {
                module.machines[0].structural_parameters[0].access =
                    StructuralAccess::WriteOnlyBorrow
            }
            2 => module.machines[0].blocks[0].operations.swap(2, 3),
            3 => module.machines[0].blocks[0].operations.swap(1, 2),
            _ => {
                // Leave the reference live at the exit without adding an
                // overlapping write that could reject before cleanup replay.
                module.machines[0].blocks[0].operations.truncate(2);
            }
        }
        assert!(
            validate_module(&module).is_err(),
            "invalid formation or loan mutation {mutation}"
        );
    }
}

#[test]
fn shared_scalar_reference_views_keep_the_complete_cohort_frozen() {
    use semantic_vocabulary::{IntegerSign, IntegerType};
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
    let mut module = scalar_reference_view_module(scalar, scalar);
    let machine = &mut module.machines[0];
    machine.structural_places.push(reference_place(3, 5));
    let operations = &mut machine.blocks[0].operations;
    let mut second_call = operations[1].clone();
    second_call.id = operation_id(6);
    let OperationResult::Scalar(result) = &mut second_call.result else {
        unreachable!()
    };
    result.id = value_id(3);
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut second_call.kind
    else {
        unreachable!()
    };
    structural_arguments[0].place = place_id(3);
    operations.insert(
        1,
        establish(
            5,
            3,
            StructuralArgument {
                place: place_id(1),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            },
        ),
    );
    operations.insert(3, second_call);
    operations.insert(5, release(7, 3));
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("shared siblings retain one backing until both are released");

    let mut early_write = module.clone();
    early_write.machines[0].blocks[0].operations.swap(5, 6);
    rejects_custody(&early_write);

    let mut exclusive_formation = module.clone();
    let OperationKind::EstablishReference { source } =
        &mut exclusive_formation.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    source.access = StructuralAccess::MutableBorrow;
    rejects_custody(&exclusive_formation);

    let mut missing_release = module;
    missing_release.machines[0].blocks[0].operations.truncate(5);
    assert!(matches!(
        validate_module(&missing_release),
        Err(ModuleError::UnitReturnAffineDiscardsMismatch { .. })
    ));
}
