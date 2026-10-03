//! Returned descriptor loans are checked from Terminal bodies, independently of
//! source borrow checking. Moving one store across the last observation changes
//! legality without changing the store, callee, backing or result signature.
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId,
    ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, ByteSequenceCarrier, MachineContract, Operation, OperationKind, OperationResult,
    ProofBundle, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration,
};
use terminal_verifier::{ModuleError, validate_module, verify_module};

fn place(value: u64) -> PlaceId {
    PlaceId::new(value).unwrap()
}
fn machine(value: u64) -> MachineId {
    MachineId::new(value).unwrap()
}
fn structure(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}
fn scalar(bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap())
}
fn value(identity: u64, bits: u16) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(identity).unwrap(),
        scalar_type: scalar(bits),
        qualifications: Default::default(),
    }
}
fn parameter(
    access: StructuralAccess,
    structural_type: StructuralTypeId,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(1),
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}
fn body(
    identity: u64,
    access: StructuralAccess,
    structural_type: StructuralTypeId,
) -> TerminalMachine {
    let entry = BlockId::new(1).unwrap();
    TerminalMachine {
        id: machine(identity),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![parameter(access, structural_type)],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: place(1),
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
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(identity).unwrap(),
            crash_routes: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}
fn declare_result(machine: &mut TerminalMachine, source: PlaceId) {
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: place(9),
        structural_type: structure(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place(9),
        kind: StructuralPlaceKind::Result,
    });
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: EdgeId::new(1).unwrap(),
        source,
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
}
fn result() -> OperationResult {
    OperationResult::Structural(StructuralOperationResult {
        place: place(2),
        structural_type: structure(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    })
}
fn operation(identity: u64, result: OperationResult, kind: OperationKind) -> Operation {
    Operation {
        id: OperationId::new(identity).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result,
        kind,
    }
}
fn add_call(caller: &mut TerminalMachine, target: u64) {
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place(2),
        kind: StructuralPlaceKind::OperationResult {
            producer: OperationId::new(1).unwrap(),
            structural_type: structure(1),
        },
    });
    caller.blocks[0].operations.push(operation(
        1,
        result(),
        OperationKind::CallStructuralWithScalarArguments {
            callee: machine(target),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place(1),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            }],
            claim_transfers: Vec::new(),
            returned_claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    ));
}
fn unique(mut body: TerminalMachine) -> TerminalMachine {
    let offset = body.id.get() * 100;
    let remap = |identity: PlaceId| place(identity.get() + offset);
    let operation_identity =
        |identity: OperationId| OperationId::new(identity.get() + offset).unwrap();
    body.entry = BlockId::new(body.entry.get() + offset).unwrap();
    for parameter in &mut body.parameters {
        parameter.id = ValueId::new(parameter.id.get() + offset).unwrap();
    }
    for parameter in &mut body.structural_parameters {
        parameter.place = remap(parameter.place);
    }
    if let TerminalMachineResult::Structural(result) = &mut body.result {
        result.place = remap(result.place);
    }
    for declaration in &mut body.structural_places {
        declaration.id = remap(declaration.id);
        if let StructuralPlaceKind::OperationResult { producer, .. } = &mut declaration.kind {
            *producer = operation_identity(*producer);
        }
    }
    for block in &mut body.blocks {
        block.id = BlockId::new(block.id.get() + offset).unwrap();
        for operation in &mut block.operations {
            operation.id = operation_identity(operation.id);
            match &mut operation.result {
                OperationResult::Scalar(result) => {
                    result.id = ValueId::new(result.id.get() + offset).unwrap()
                }
                OperationResult::Structural(result) => result.place = remap(result.place),
                OperationResult::Unit => {}
            }
            operation
                .kind
                .map_structural_uses(&mut |source| remap(source));
            match &mut operation.kind {
                OperationKind::WriteOnlyPrimitiveStore { value, .. } => {
                    *value = ValueId::new(value.get() + offset).unwrap()
                }
                OperationKind::EstablishElementView { destination, .. } => {
                    *destination = remap(*destination)
                }
                _ => {}
            }
        }
        match &mut block.terminator {
            Terminator::ReturnUnit { edge, .. } => {
                *edge = EdgeId::new(edge.get() + offset).unwrap()
            }
            Terminator::ReturnStructural { edge, source, .. } => {
                *edge = EdgeId::new(edge.get() + offset).unwrap();
                *source = remap(*source);
            }
            _ => unreachable!("fixture has only normal returns"),
        }
    }
    body
}

fn fixture() -> TerminalModule {
    let mut module = TerminalModule::for_entry(machine(1));
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: structure(1),
            identity: "Bytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: None,
            }),
        },
        StructuralTypeDeclaration {
            id: structure(2),
            identity: "Backing".into(),
            shape: StructuralTypeShape::FixedArray {
                element: structure(3),
                length: 4,
            },
        },
        StructuralTypeDeclaration {
            id: structure(3),
            identity: "u8".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar(8)),
        },
    ];
    let mut leaf = body(3, StructuralAccess::SharedBorrow, structure(1));
    declare_result(&mut leaf, place(1));
    let mut relay = body(2, StructuralAccess::SharedBorrow, structure(1));
    add_call(&mut relay, 3);
    declare_result(&mut relay, place(2));
    let mut caller = body(1, StructuralAccess::MutableBorrow, structure(2));
    caller.parameters.push(value(1, 8));
    add_call(&mut caller, 2);
    caller.blocks[0].operations.extend([
        operation(
            2,
            OperationResult::Scalar(value(2, 64)),
            OperationKind::ByteSequenceLength { source: place(2) },
        ),
        operation(
            3,
            OperationResult::Unit,
            OperationKind::WriteOnlyPrimitiveStore {
                destination: place(1),
                path: vec![StructuralPathSegment::FixedIndex(0)],
                value: ValueId::new(1).unwrap(),
            },
        ),
    ]);
    module.machines = vec![unique(caller), unique(relay), unique(leaf)];
    module
}

#[test]
fn returned_descriptor_releases_backing_after_its_last_observation() {
    let module = fixture();
    verify_module(
        &module,
        &ProofBundle::default(),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("nested returned descriptor dies before the same backing is written");
}

#[test]
fn returned_descriptor_rejects_mutation_before_its_last_observation() {
    let mut module = fixture();
    verify_module(
        &module,
        &ProofBundle::default(),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("unchanged control verifies before tampering");
    module.machines[0].blocks[0].operations.swap(1, 2);
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::BorrowedDescriptorLoanDisturbed { .. })
        ),
        "the nested returned descriptor pins the original array, not a private descriptor home"
    );
}

#[test]
fn descriptor_return_rejects_owned_backing_despite_identical_result_type() {
    let mut module = fixture();
    let mut callee = body(3, StructuralAccess::Owned, structure(2));
    declare_result(&mut callee, place(2));
    callee.structural_places.push(StructuralPlaceDeclaration {
        id: place(2),
        kind: StructuralPlaceKind::OperationResult {
            producer: OperationId::new(1).unwrap(),
            structural_type: structure(1),
        },
    });
    callee.blocks[0].operations.push(operation(
        1,
        result(),
        OperationKind::EstablishElementView {
            region: terminal_psi::ElementViewRegion::WholeCollection,
            destination: place(2),
            source: StructuralArgument {
                place: place(1),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            },
            element: structure(3),
        },
    ));
    let Terminator::ReturnStructural { source, .. } = &mut callee.blocks[0].terminator else {
        unreachable!()
    };
    *source = place(2);
    module.entry = callee.id;
    module.machines = vec![unique(callee)];
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::ByteSequenceSubsliceReturnUnsupported { .. })
        ),
        "returning a descriptor cannot extend activation-owned storage"
    );
}
fn shared_slot_case_return(selected_case: u64) -> TerminalModule {
    use semantic_vocabulary::{StructuralCaseId, StructuralFieldId};
    use terminal_psi::{
        BindingRelevance, RecordFieldInitializer, RecordFieldValue, StructuralCaseDeclaration,
        StructuralFieldDeclaration, StructuralFieldType,
    };
    let mut module = TerminalModule::for_entry(machine(1));
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: structure(1),
            identity: "shared bytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: None,
            }),
        },
        StructuralTypeDeclaration {
            id: structure(2),
            identity: "retained".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: "bytes".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::ByteSequence(
                        ByteSequenceCarrier::BorrowedView {
                            access: Some(StructuralAccess::SharedBorrow),
                        },
                    ),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: structure(3),
            identity: "result".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![
                    StructuralCaseDeclaration {
                        id: StructuralCaseId::new(1).unwrap(),
                        identity: "empty".into(),
                        fields: Vec::new(),
                    },
                    StructuralCaseDeclaration {
                        id: StructuralCaseId::new(2).unwrap(),
                        identity: "present".into(),
                        fields: vec![StructuralFieldDeclaration {
                            id: StructuralFieldId::new(2).unwrap(),
                            identity: "retained".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(structure(2)),
                        }],
                    },
                ],
            },
        },
        StructuralTypeDeclaration {
            id: structure(4),
            identity: "local bytes".into(),
            shape: StructuralTypeShape::FixedArray {
                element: structure(5),
                length: 4,
            },
        },
        StructuralTypeDeclaration {
            id: structure(5),
            identity: "u8".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar(8)),
        },
    ];
    let mut callee = body(1, StructuralAccess::SharedBorrow, structure(1));
    let mut append = |identity, home, structural_type, kind| {
        callee.structural_places.push(StructuralPlaceDeclaration {
            id: place(home),
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(identity).unwrap(),
                structural_type: structure(structural_type),
            },
        });
        callee.blocks[0].operations.push(operation(
            identity,
            OperationResult::Structural(StructuralOperationResult {
                place: place(home),
                structural_type: structure(structural_type),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            kind,
        ));
    };
    if selected_case == 2 {
        append(
            10,
            2,
            2,
            OperationKind::EstablishRecord {
                fields: vec![RecordFieldInitializer {
                    field: StructuralFieldId::new(1).unwrap(),
                    value: RecordFieldValue::Structural(StructuralArgument {
                        place: place(1),
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    }),
                }],
            },
        );
    }
    append(
        20,
        3,
        3,
        OperationKind::EstablishStructuralCase {
            result_case: StructuralCaseId::new(selected_case).unwrap(),
            fields: if selected_case == 2 {
                vec![RecordFieldInitializer {
                    field: StructuralFieldId::new(2).unwrap(),
                    value: RecordFieldValue::Structural(StructuralArgument {
                        place: place(2),
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    }),
                }]
            } else {
                Vec::new()
            },
        },
    );
    declare_result(&mut callee, place(3));
    let TerminalMachineResult::Structural(result) = &mut callee.result else {
        unreachable!()
    };
    result.structural_type = structure(3);
    result.multiplicity = StructuralMultiplicity::Affine;
    module.machines = vec![callee];
    module
}

#[test]
fn returned_case_shared_slots_verify_populated_and_inactive_payloads() {
    for selected_case in [1, 2] {
        let module = shared_slot_case_return(selected_case);
        assert!(
            terminal_semantics::descriptor_results::return_slots_have_external_backing(
                &module,
                &module.machines[0],
                place(3),
            )
        );
        verify_module(
            &module,
            &ProofBundle::default(),
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("an owned case retains its exact shared-slot backing or proved absent payload");
    }
}

#[test]
fn returned_case_shared_slots_reject_local_and_unknown_backing() {
    use terminal_psi::RecordFieldValue;
    let mut module = shared_slot_case_return(2);
    let callee = &mut module.machines[0];
    callee.structural_parameters[0].access = StructuralAccess::Owned;
    callee.structural_parameters[0].structural_type = structure(4);
    callee.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    callee.structural_places.push(StructuralPlaceDeclaration {
        id: place(6),
        kind: StructuralPlaceKind::OperationResult {
            producer: OperationId::new(5).unwrap(),
            structural_type: structure(1),
        },
    });
    let mut view_result = result();
    let OperationResult::Structural(view_result) = &mut view_result else {
        unreachable!()
    };
    view_result.place = place(6);
    let view_result = view_result.clone();
    let OperationKind::EstablishRecord { fields } = &mut callee.blocks[0].operations[0].kind else {
        unreachable!()
    };
    let RecordFieldValue::Structural(argument) = &mut fields[0].value else {
        unreachable!()
    };
    argument.place = place(6);
    callee.blocks[0].operations.insert(
        0,
        operation(
            5,
            OperationResult::Structural(view_result),
            OperationKind::EstablishElementView {
                region: terminal_psi::ElementViewRegion::WholeCollection,
                destination: place(6),
                source: StructuralArgument {
                    place: place(1),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                },
                element: structure(5),
            },
        ),
    );
    let Terminator::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &mut callee.blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.push(place(1));
    assert!(
        !terminal_semantics::descriptor_results::return_slots_have_external_backing(
            &module,
            &module.machines[0],
            place(3),
        )
    );
    assert!(
        validate_module(&module).is_err(),
        "a returned slot cannot outlive owned backing"
    );

    let mut module = shared_slot_case_return(2);
    let OperationKind::EstablishRecord { fields } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    fields.clear();
    assert!(
        !terminal_semantics::descriptor_results::return_slots_have_external_backing(
            &module,
            &module.machines[0],
            place(3),
        )
    );
    assert!(
        validate_module(&module).is_err(),
        "an active omitted descriptor is unknown, not empty"
    );
}

#[test]
fn returned_case_shared_slots_preserve_claim_signature_and_access_checks() {
    let original = shared_slot_case_return(2);
    verify_module(
        &original,
        &ProofBundle::default(),
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    let mut claims = original.clone();
    let Terminator::ReturnStructural {
        returned_claims, ..
    } = &mut claims.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    returned_claims.push(semantic_vocabulary::ClaimId::new(1).unwrap());
    assert!(matches!(
        validate_module(&claims),
        Err(ModuleError::StructuralReturnClaimSetMismatch { .. })
    ));
    let mut signature = original;
    let TerminalMachineResult::Structural(result) = &mut signature.machines[0].result else {
        unreachable!()
    };
    result.structural_type = structure(2);
    assert!(validate_module(&signature).is_err());

    // Neither an explicit mutable field nor a structural descriptor whose
    // access lives in its producer can acquire shared permission from shape.
    for field_type in [
        terminal_psi::StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::MutableBorrow),
        }),
        terminal_psi::StructuralFieldType::Structural(structure(1)),
    ] {
        let mut unproved_access = shared_slot_case_return(1);
        let StructuralTypeShape::Record { fields } = &mut unproved_access.structural_types[1].shape
        else {
            unreachable!()
        };
        fields[0].field_type = field_type;
        assert!(matches!(
            validate_module(&unproved_access),
            Err(ModuleError::NonCanonicalStructuralReturnClaims { .. })
        ));
    }
}

#[test]
fn returned_case_shared_slots_do_not_turn_borrowed_aggregate_into_owned_result() {
    let mut module = shared_slot_case_return(2);
    let callee = &mut module.machines[0];
    callee.structural_parameters[0].structural_type = structure(3);
    callee.blocks[0].operations.clear();
    callee.structural_places.retain(|row| {
        matches!(
            row.kind,
            StructuralPlaceKind::Parameter { .. } | StructuralPlaceKind::Result
        )
    });
    let TerminalMachineResult::Structural(result) = &mut callee.result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Unrestricted;
    let Terminator::ReturnStructural { source, .. } = &mut callee.blocks[0].terminator else {
        unreachable!()
    };
    *source = place(1);
    assert!(
        terminal_semantics::descriptor_results::return_origins(
            &module,
            &module.machines[0],
            place(1)
        )
        .is_none()
    );
    assert!(
        terminal_semantics::descriptor_results::return_slots_have_external_backing(
            &module,
            &module.machines[0],
            place(1)
        )
    );
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::NonCanonicalStructuralReturnClaims { .. })
        ),
        "external nested backing does not grant ownership of a borrowed aggregate carrier"
    );
}
