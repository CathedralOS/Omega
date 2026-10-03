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
