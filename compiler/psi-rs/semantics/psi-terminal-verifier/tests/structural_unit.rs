use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace,
    ContentTerm, ContractId, EdgeId, MachineId, ObligationId, OperationId, PlaceId, Proposition,
    ServiceId, StructuralDomainId, StructuralPlaceKind, StructuralTypeId,
};
use psi_proof_kernel::AdmissionProfile;
use psi_terminal::{
    Block, BoundaryMachineDeclaration, ClaimContentProjection, ClaimSettlement, ClaimTransfer,
    ContentEntryClaim, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard,
    EntryClaim, MachineContract, Operation, OperationKind, OperationResult, ServiceDeclaration,
    StructuralArgument, StructuralDomainDeclaration, StructuralDomainRequirement,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, VocabularyMarker,
};
use psi_terminal_verifier::{
    ModuleError, ProofBundle, reconstruct_operation_obligations, validate_module, verify_module,
};

#[test]
fn hard_root_unit_slice_validates_and_verifies() {
    let module = hard_root_module();

    validate_module(&module).expect("structural Unit call/boundary/effect slice validates");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("structural Unit operations require no producer-authored structural proof evidence");
}

#[test]
fn unit_call_requirements_remain_callee_contract_obligations() {
    let mut module = hard_root_module();
    module.machines[1].contract.requires = vec![Proposition::Truth];
    let OperationKind::CallUnit {
        requirement_obligations,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    requirement_obligations.push(obligation_id(1));

    let obligations = reconstruct_operation_obligations(&module).expect("Unit call obligations");
    assert_eq!(obligations.len(), 1);
    assert_eq!(obligations[0].obligation.id, obligation_id(1));
    assert_eq!(obligations[0].obligation.proposition, Proposition::Truth);
}

#[test]
fn unit_call_checks_structural_type_qualification_and_transfer_shape() {
    let mut wrong_type = hard_root_module();
    wrong_type.machines[0].structural_parameters[0].structural_type = structural_type_id(2);
    wrong_type.machines[0].structural_parameters[0]
        .qualifications
        .clear();
    assert_eq!(
        validate_module(&wrong_type).unwrap_err(),
        ModuleError::StructuralArgumentTypeMismatch {
            operation: operation_id(1),
            argument_index: 0,
            expected: structural_type_id(1),
            actual: structural_type_id(2),
        }
    );

    let mut missing_qualification = hard_root_module();
    missing_qualification.machines[0].structural_parameters[0]
        .qualifications
        .clear();
    assert_eq!(
        validate_module(&missing_qualification).unwrap_err(),
        ModuleError::StructuralArgumentMissingQualification {
            operation: operation_id(1),
            argument_index: 0,
            domain: domain_id(1),
        }
    );

    let mut missing_transfer = hard_root_module();
    unit_call_mut(&mut missing_transfer).clear();
    assert_eq!(
        validate_module(&missing_transfer).unwrap_err(),
        ModuleError::UnitCallClaimTransferCountMismatch {
            operation: operation_id(1),
            expected: 1,
            actual: 0,
        }
    );
}

#[test]
fn structural_calls_preserve_optional_affine_claim_custody() {
    let mut dropped_at_call = hard_root_module();
    for machine in &mut dropped_at_call.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    }
    dropped_at_call.machines[1].entry_claims.clear();
    unit_call_mut(&mut dropped_at_call).clear();
    assert_eq!(
        validate_module(&dropped_at_call).unwrap_err(),
        ModuleError::UnitCallClaimPresenceMismatch {
            operation: operation_id(1),
            argument_index: 0,
        }
    );

    let mut minted_at_call = hard_root_module();
    for machine in &mut minted_at_call.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    }
    minted_at_call.machines[0].entry_claims.clear();
    unit_call_mut(&mut minted_at_call).clear();
    assert_eq!(
        validate_module(&minted_at_call).unwrap_err(),
        ModuleError::UnitCallClaimPresenceMismatch {
            operation: operation_id(1),
            argument_index: 0,
        }
    );

    let mut settled_at_boundary = hard_root_module();
    for machine in &mut settled_at_boundary.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    }
    settled_at_boundary.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    validate_module(&settled_at_boundary)
        .expect("a proof-visible affine claim is settled with its consumed owned place");

    boundary_call_mut(&mut settled_at_boundary).0.clear();
    assert_eq!(
        validate_module(&settled_at_boundary).unwrap_err(),
        ModuleError::BoundaryClaimSettlementMismatch(operation_id(3))
    );
}

#[test]
fn unit_calls_preserve_exact_content_claim_shape() {
    let mut matching = hard_root_module();
    matching.machines[0].content_entry_claims = vec![content_entry_claim(place_id(1))];
    matching.machines[1].content_entry_claims = vec![content_entry_claim(place_id(2))];
    validate_module(&matching).expect("an ordinary structural transfer preserves exact content");

    let mut dropped = matching.clone();
    dropped.machines[1].content_entry_claims.clear();
    assert_eq!(
        validate_module(&dropped).unwrap_err(),
        ModuleError::UnitCallContentClaimMismatch {
            operation: operation_id(1),
            argument_index: 0,
        }
    );

    let mut minted = matching.clone();
    minted.machines[0].content_entry_claims.clear();
    assert_eq!(
        validate_module(&minted).unwrap_err(),
        ModuleError::UnitCallContentClaimMismatch {
            operation: operation_id(1),
            argument_index: 0,
        }
    );

    let mut redirected = matching.clone();
    redirected.machines[1].content_entry_claims[0]
        .input
        .segments
        .push(ContentPlaceSegment::Field("payload".to_owned()));
    assert_eq!(
        validate_module(&redirected).unwrap_err(),
        ModuleError::UnitCallContentClaimMismatch {
            operation: operation_id(1),
            argument_index: 0,
        }
    );

    let mut reinterpreted = matching;
    reinterpreted.machines[1].content_entry_claims[0].projections[0]
        .projection
        .projection_fingerprint ^= 1;
    assert_eq!(
        validate_module(&reinterpreted).unwrap_err(),
        ModuleError::UnitCallContentClaimMismatch {
            operation: operation_id(1),
            argument_index: 0,
        }
    );
}

#[test]
fn boundary_call_checks_qualification_settlement_and_obligation_absence() {
    let mut missing_qualification = hard_root_module();
    missing_qualification.machines[1].structural_parameters[0]
        .qualifications
        .clear();
    assert_eq!(
        validate_module(&missing_qualification).unwrap_err(),
        ModuleError::BoundaryArgumentMissingQualification {
            operation: operation_id(3),
            argument_index: 0,
            domain: domain_id(1),
        }
    );

    let mut missing_settlement = hard_root_module();
    boundary_call_mut(&mut missing_settlement).0.clear();
    assert_eq!(
        validate_module(&missing_settlement).unwrap_err(),
        ModuleError::BoundaryClaimSettlementMismatch(operation_id(3))
    );

    let mut minted_obligation = hard_root_module();
    boundary_call_mut(&mut minted_obligation)
        .1
        .push(obligation_id(1));
    assert_eq!(
        validate_module(&minted_obligation).unwrap_err(),
        ModuleError::BoundaryStructuralRequirementsMintObligations(operation_id(3))
    );
}

#[test]
fn claims_are_linear_across_unit_operations_and_return() {
    let mut reused = hard_root_module();
    reused.machines[0].blocks[0].operations.push(Operation {
        id: operation_id(4),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCallUnit {
            boundary: boundary_id(1),
            structural_arguments: vec![StructuralArgument { place: place_id(1) }],
            claim_settlements: vec![ClaimSettlement {
                claim: claim_id(1),
                argument_index: 0,
            }],
            requirement_obligations: Vec::new(),
        },
    });
    assert_eq!(
        validate_module(&reused).unwrap_err(),
        ModuleError::ClaimNotLiveAtOperation {
            operation: operation_id(4),
            claim: claim_id(1),
        }
    );

    let mut leaked = hard_root_module();
    leaked.machines[1].blocks[0].operations.truncate(1);
    assert_eq!(
        validate_module(&leaked).unwrap_err(),
        ModuleError::LiveLinearClaimAtUnitReturn {
            machine: machine_id(2),
            block: block_id(2),
            claim: claim_id(1),
        }
    );
}

#[test]
fn entry_claims_are_dense_in_each_machine_local_namespace() {
    let mut module = hard_root_module();
    assert_eq!(module.machines[0].entry_claims[0].claim, claim_id(1));
    assert_eq!(module.machines[1].entry_claims[0].claim, claim_id(1));
    validate_module(&module).expect("each machine starts its claim namespace at one");

    module.machines[1].entry_claims[0].claim = claim_id(2);
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::NonDenseStructuralEntryClaim {
            machine: machine_id(2),
            expected: claim_id(1),
            actual: claim_id(2),
        }
    );
}

#[test]
fn affine_structural_arguments_transfer_at_most_once() {
    let mut repeated = hard_root_module();
    for machine in &mut repeated.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        machine.entry_claims.clear();
    }
    unit_call_mut(&mut repeated).clear();
    repeated.machines[1].blocks[0].operations.truncate(1);
    let mut second_call = repeated.machines[0].blocks[0].operations[0].clone();
    second_call.id = operation_id(4);
    repeated.machines[0].blocks[0].operations.push(second_call);
    assert_eq!(
        validate_module(&repeated).unwrap_err(),
        ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: operation_id(4),
            place: place_id(1),
        }
    );

    for machine in &mut repeated.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
    }
    validate_module(&repeated).expect("unrestricted structural arguments remain reusable");

    let mut repeated_boundary = hard_root_module();
    repeated_boundary.machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    repeated_boundary.machines[0].entry_claims.clear();
    repeated_boundary.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    let boundary_call = Operation {
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCallUnit {
            boundary: boundary_id(1),
            structural_arguments: vec![StructuralArgument { place: place_id(1) }],
            claim_settlements: Vec::new(),
            requirement_obligations: Vec::new(),
        },
    };
    let mut second_boundary_call = boundary_call.clone();
    second_boundary_call.id = operation_id(4);
    repeated_boundary.machines[0].blocks[0].operations = vec![boundary_call, second_boundary_call];
    assert_eq!(
        validate_module(&repeated_boundary).unwrap_err(),
        ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: operation_id(4),
            place: place_id(1),
        }
    );
}

#[test]
fn port_write_requires_a_declared_reachable_service_and_preserves_claims() {
    let mut outside_ceiling = hard_root_module();
    outside_ceiling.services.push(ServiceDeclaration {
        id: service_id(2),
        identity: "DebugIo".into(),
        parents: Vec::new(),
    });
    let OperationKind::PortWrite { service, .. } =
        &mut outside_ceiling.machines[1].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *service = service_id(2);
    assert_eq!(
        validate_module(&outside_ceiling).unwrap_err(),
        ModuleError::OperationServiceOutsidePublishedCeiling {
            operation: operation_id(2),
            service: service_id(2),
        }
    );
}

#[test]
fn crash_frontier_is_the_exact_live_frontier_after_transfer() {
    let mut module = hard_root_module();
    module.machines[0].contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    module.machines[0].blocks[0].terminator = Terminator::Crash {
        edge: edge_id(1),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    validate_module(&module).expect("the transferred claim is absent from the crash frontier");

    let Terminator::Crash {
        frontier_lower_bound,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    frontier_lower_bound.push(claim_id(1));
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::CrashFrontierMismatch { block: block_id(1) }
    );
}

#[test]
fn unit_calls_preserve_exact_crash_routes_and_remain_acyclic() {
    let mut crash_erasing = hard_root_module();
    crash_erasing.machines[1].contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    assert_eq!(
        validate_module(&crash_erasing).unwrap_err(),
        ModuleError::CallCrashContinuationsMismatch {
            operation: operation_id(1),
            callee: machine_id(2),
        }
    );

    let mut recursive = hard_root_module();
    recursive.machines[1].blocks[0].operations = vec![Operation {
        id: operation_id(2),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(2),
            structural_arguments: vec![StructuralArgument { place: place_id(2) }],
            claim_transfers: vec![ClaimTransfer {
                claim: claim_id(1),
                argument_index: 0,
            }],
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }];
    assert_eq!(
        validate_module(&recursive).unwrap_err(),
        ModuleError::RecursiveCallSliceNotYetSupported(machine_id(2))
    );
}

#[test]
fn unit_call_crash_routes_substitute_structural_parameters() {
    let mut module = hard_root_module();
    let callee_route = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            content_predicate(place_id(2)),
        ))],
    };
    let caller_route = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            content_predicate(place_id(1)),
        ))],
    };
    module.machines[0].contract.crash_routes = vec![caller_route.clone()];
    module.machines[1].contract.crash_routes = vec![callee_route.clone()];
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *crash_continuations = vec![caller_route];
    validate_module(&module).expect("callee structural crash places substitute to caller places");

    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *crash_continuations = vec![callee_route];
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::CallCrashContinuationsMismatch {
            operation: operation_id(1),
            callee: machine_id(2),
        }
    );
}

fn hard_root_module() -> TerminalModule {
    let resource = StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "PortResource".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    let other = StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "OtherResource".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    let pending = StructuralDomainDeclaration {
        id: domain_id(1),
        identity: "Pending".into(),
        carrier: resource.id,
    };
    let port_io = ServiceDeclaration {
        id: service_id(1),
        identity: "PortIo".into(),
        parents: Vec::new(),
    };
    let mut boundary_parameter = structural_parameter(place_id(9));
    boundary_parameter.qualifications.clear();
    let boundary = BoundaryMachineDeclaration {
        id: boundary_id(1),
        identity: "settle_port".into(),
        attachment: None,
        structural_parameters: vec![boundary_parameter],
        requires: vec![StructuralDomainRequirement {
            argument_index: 0,
            domain: pending.id,
        }],
        published_service_ceiling: vec![port_io.id],
    };

    let caller = TerminalMachine {
        id: machine_id(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![structural_parameter(place_id(1))],
        result: TerminalMachineResult::Unit,
        structural_places: vec![structural_place(place_id(1))],
        entry_claims: vec![EntryClaim {
            claim: claim_id(1),
            input: place_id(1),
        }],
        published_service_ceiling: vec![port_io.id],
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation_id(1),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    callee: machine_id(2),
                    structural_arguments: vec![StructuralArgument { place: place_id(1) }],
                    claim_transfers: vec![ClaimTransfer {
                        claim: claim_id(1),
                        argument_index: 0,
                    }],
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnit { edge: edge_id(1) },
        }],
        contract: empty_contract(contract_id(1)),
    };

    let callee = TerminalMachine {
        id: machine_id(2),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![structural_parameter(place_id(2))],
        result: TerminalMachineResult::Unit,
        structural_places: vec![structural_place(place_id(2))],
        entry_claims: vec![EntryClaim {
            claim: claim_id(1),
            input: place_id(2),
        }],
        published_service_ceiling: vec![port_io.id],
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(2),
        blocks: vec![Block {
            id: block_id(2),
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: operation_id(2),
                    result: OperationResult::Unit,
                    kind: OperationKind::PortWrite {
                        service: port_io.id,
                        port: 0x3f8,
                        value: b'X',
                    },
                },
                Operation {
                    id: operation_id(3),
                    result: OperationResult::Unit,
                    kind: OperationKind::BoundaryCallUnit {
                        boundary: boundary.id,
                        structural_arguments: vec![StructuralArgument { place: place_id(2) }],
                        claim_settlements: vec![ClaimSettlement {
                            claim: claim_id(1),
                            argument_index: 0,
                        }],
                        requirement_obligations: Vec::new(),
                    },
                },
            ],
            terminator: Terminator::ReturnUnit { edge: edge_id(2) },
        }],
        contract: empty_contract(contract_id(2)),
    };

    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller.id,
        structural_types: vec![resource, other],
        structural_domains: vec![pending],
        services: vec![port_io],
        boundary_machines: vec![boundary],
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![caller, callee],
    }
}

fn structural_parameter(place: PlaceId) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: vec![domain_id(1)],
    }
}

fn content_entry_claim(root: PlaceId) -> ContentEntryClaim {
    ContentEntryClaim {
        claim: claim_id(1),
        input: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root,
            segments: Vec::new(),
        },
        projections: vec![ClaimContentProjection {
            projection: ContentProjectionIdentity {
                domain: ContentDomainId::new(1).expect("content domain"),
                projection_fingerprint: 0xfeed,
            },
            algebra: ContentAlgebra {
                kind: ContentAlgebraKind::CountedQuantity,
                parameter: "Acknowledgement".to_owned(),
            },
        }],
    }
}

fn structural_place(id: PlaceId) -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }
}

fn empty_contract(id: ContractId) -> MachineContract {
    MachineContract {
        id,
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
    }
}

fn content_predicate(root: PlaceId) -> Proposition {
    let projection = ContentProjectionIdentity {
        domain: ContentDomainId::new(1).expect("content domain"),
        projection_fingerprint: 1,
    };
    let projected = |field: &str| ContentTerm::Projection {
        projection,
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root,
            segments: vec![ContentPlaceSegment::Field(field.into())],
        },
    };
    Proposition::ContentConservation(psi_core::ContentConservation::new(
        ContentAlgebra {
            kind: ContentAlgebraKind::CountedQuantity,
            parameter: "Byte".into(),
        },
        projected("left"),
        projected("right"),
    ))
}

fn unit_call_mut(module: &mut TerminalModule) -> &mut Vec<ClaimTransfer> {
    let OperationKind::CallUnit {
        claim_transfers, ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    claim_transfers
}

fn boundary_call_mut(
    module: &mut TerminalModule,
) -> (&mut Vec<ClaimSettlement>, &mut Vec<ObligationId>) {
    let OperationKind::BoundaryCallUnit {
        claim_settlements,
        requirement_obligations,
        ..
    } = &mut module.machines[1].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    (claim_settlements, requirement_obligations)
}

macro_rules! id_fn {
    ($name:ident, $type:ty) => {
        fn $name(raw: u64) -> $type {
            <$type>::new(raw).expect("nonzero test identity")
        }
    };
}

id_fn!(block_id, BlockId);
id_fn!(boundary_id, BoundaryMachineId);
id_fn!(claim_id, ClaimId);
id_fn!(contract_id, ContractId);
id_fn!(edge_id, EdgeId);
id_fn!(machine_id, MachineId);
id_fn!(obligation_id, ObligationId);
id_fn!(operation_id, OperationId);
id_fn!(place_id, PlaceId);
id_fn!(service_id, ServiceId);
id_fn!(structural_type_id, StructuralTypeId);
id_fn!(domain_id, StructuralDomainId);
