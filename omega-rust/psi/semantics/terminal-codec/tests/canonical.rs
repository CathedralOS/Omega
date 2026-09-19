//! Fixtures shared by the canonical codec tests: provider attachment,
//! nominal affine, unit and entry claim fixtures.

#[path = "canonical/affine_and_claim_round_trips.rs"]
mod affine_and_claim_round_trips;
#[path = "canonical/borrowed_storage_windows.rs"]
mod borrowed_storage_windows;
#[path = "canonical/boundary_crashes.rs"]
mod boundary_crashes;
#[path = "canonical/bounded_integer_fields.rs"]
mod bounded_integer_fields;
#[path = "canonical/contract_fields.rs"]
mod contract_fields;
#[path = "canonical/operation_crash_contracts.rs"]
mod operation_crash_contracts;
#[path = "canonical/owned_integer_fields.rs"]
mod owned_integer_fields;
#[path = "canonical/scalar_array_arguments.rs"]
mod scalar_array_arguments;
#[path = "canonical/scalar_block_invariants.rs"]
mod scalar_block_invariants;
#[path = "canonical/scalar_case_fields.rs"]
mod scalar_case_fields;
#[path = "canonical/scalar_qualifications.rs"]
mod scalar_qualifications;
#[path = "canonical/structural_and_proposition_rows.rs"]
mod structural_and_proposition_rows;
#[path = "canonical/structural_call_results.rs"]
mod structural_call_results;
#[path = "canonical/suspension_and_scalar_round_trips.rs"]
mod suspension_and_scalar_round_trips;

use semantic_vocabulary::{
    BlockId, BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, ContentAlgebra,
    ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, Proposition, PropositionId, ScalarTerm, ScalarType, ServiceId,
    StructuralCaseId, StructuralDomainId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
    SuspensionCrossingId, ValueId,
};
use terminal_codec::{
    CodecError, decode_module, encode_module, semantic_fingerprint, terminal_psi_identity,
};

use terminal_psi::{
    BindingRelevance, Block, BoundaryMachineDeclaration, ClaimContentProjection, ClaimTransfer,
    CompletionReceipt, ContentEntryClaim, ContentIdentityReshuffle, ContentPartitionComposition,
    ContentPlaceSubstitution, ContractClause, CrashCause, CrashRouteBucket, CrashRouteGuard,
    EntryClaim, MachineContract, NominalAffineCleanup, Operation, OperationKind, OperationResult,
    ServiceDeclaration, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralCaseDeclaration, StructuralDomainDeclaration, StructuralDomainRequirement,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralResultClaimBinding, StructuralResultClaimTransfer,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge,
    TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult, TerminalModule,
    TerminalProofRankingRelation, TerminalProofRecursiveCallSite, TerminalProofRecursiveComponent,
    TerminalProofRecursiveEdge, TerminalProofRecursiveField, TerminalProofRecursiveMember,
    TerminalProofRecursiveType, TerminalRankedScc, Terminator, ValueDeclaration, VocabularyMarker,
};

fn unused_provider_attachment_fixture() -> TerminalModule {
    let mut module = unit_fixture();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "example::Main".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: structural_field_id(1),
                identity: "console".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Erased {
                    type_identity: "named(name(example::ConsoleProvider))".into(),
                },
            }],
        },
    });
    module.machines[0].attachment = Some(structural_type_id(1));
    module.boundary_machines.push(BoundaryMachineDeclaration {
        fixed_service_reach: Vec::new(),
        crash_routes: Vec::new(),
        id: boundary_machine_id(1),
        identity: "example::Console::write".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    module
}

fn provider_attachment_root() -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id: place_id(1),
        kind: StructuralPlaceKind::ProviderAttachment {
            attachment: structural_type_id(1),
            field: structural_field_id(1),
            boundary: boundary_machine_id(1),
        },
    }
}

fn provider_boundary_call() -> Operation {
    Operation {
        static_reach_binding: None,
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: boundary_machine_id(1),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    }
}

fn partial_affine_fixture() -> TerminalModule {
    let pair_type = structural_type_id(1);
    let token_type = structural_type_id(2);
    let root_type = structural_type_id(3);
    let sink_type = structural_type_id(4);
    let pair_place = place_id(1);
    let token_place = place_id(2);
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: pair_type,
                identity: "example::Pair".to_owned(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        StructuralFieldDeclaration {
                            id: structural_field_id(1),
                            identity: "left".to_owned(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(token_type),
                        },
                        StructuralFieldDeclaration {
                            id: structural_field_id(2),
                            identity: "right".to_owned(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(token_type),
                        },
                    ],
                },
            },
            StructuralTypeDeclaration {
                id: token_type,
                identity: "example::Token".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: root_type,
                identity: "example::Root".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: sink_type,
                identity: "example::Sink".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(1),
                attachment: Some(root_type),
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: pair_place,
                    position: 0,
                    is_self: false,
                    structural_type: pair_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: vec![StructuralPlaceDeclaration {
                    id: pair_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                }],
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: operation_id(1),
                        result: OperationResult::Unit,
                        kind: OperationKind::CallUnit {
                            erased_arguments: Vec::new(),
                            arguments: Vec::new(),
                            callee: machine_id(2),
                            structural_arguments: vec![StructuralArgument {
                                place: pair_place,
                                access: StructuralAccess::Owned,
                                path: vec![StructuralPathSegment::Field("right".to_owned())],
                            }],
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                    }],
                    terminator: Terminator::ReturnUnitPartialAffine {
                        edge: edge_id(1),
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: vec![StructuralAffineDiscard {
                            place: pair_place,
                            path: vec![StructuralPathSegment::Field("left".to_owned())],
                            structural_type: token_type,
                        }],
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: contract_id(1),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(2),
                attachment: Some(sink_type),
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: token_place,
                    position: 0,
                    is_self: false,
                    structural_type: token_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: vec![StructuralPlaceDeclaration {
                    id: token_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                }],
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(2),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(2),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(2),
                        trivial_affine_discards: vec![token_place],
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: contract_id(2),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
        ],
    }
}

fn two_nominal_affine_fixture() -> TerminalModule {
    let mut module = nominal_affine_fixture();
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(2),
            position: 1,
            is_self: false,
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(2),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups.insert(
        0,
        NominalAffineCleanup {
            place: place_id(2),
            structural_type: structural_type_id(1),
            cleanup_machine: machine_id(2),
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        },
    );
    module
}

fn five_nominal_affine_fixture() -> TerminalModule {
    let mut module = two_nominal_affine_fixture();
    let machine = &mut module.machines[0];
    for position in 2_u32..5 {
        let place = place_id(u64::from(position + 1));
        machine
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place,
                position,
                is_self: false,
                structural_type: structural_type_id(1),
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::Parameter {
                position,
                is_self: false,
            },
        });
        let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
            &mut machine.blocks[0].terminator
        else {
            unreachable!()
        };
        cleanups.insert(
            0,
            NominalAffineCleanup {
                place,
                structural_type: structural_type_id(1),
                cleanup_machine: machine_id(2),
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
        );
    }
    module
}

fn nominal_affine_fixture() -> TerminalModule {
    let resource_type = structural_type_id(1);
    let owner_type = structural_type_id(2);
    let source_place = place_id(1);
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: resource_type,
                identity: "example::NominalResource".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: owner_type,
                identity: "example::Owner".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(1),
                attachment: Some(owner_type),
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: source_place,
                    position: 0,
                    is_self: false,
                    structural_type: resource_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: vec![StructuralPlaceDeclaration {
                    id: source_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                }],
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnitNominalAffine {
                        edge: edge_id(1),
                        cleanups: vec![NominalAffineCleanup {
                            place: source_place,
                            structural_type: resource_type,
                            cleanup_machine: machine_id(2),
                            cleanup_receiver: None,
                            requirement_obligations: Vec::new(),
                        }],
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: contract_id(1),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(2),
                attachment: Some(resource_type),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(2),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(2),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(2),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: contract_id(2),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
        ],
    }
}

fn structural_effect_fixture() -> TerminalModule {
    let resource_type = structural_type_id(1);
    let domain = structural_domain_id(1);
    let service = service_id(1);
    let caller_place = place_id(10);
    let callee_place = place_id(20);
    let structural_parameter =
        |place, position, structural_type, is_self| StructuralParameterDeclaration {
            place,
            position,
            is_self,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            access: StructuralAccess::Owned,
            qualifications: vec![domain],
            projected_qualifications: Vec::new(),
        };
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(100),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: resource_type,
                identity: "example::OccurrenceToken".to_owned(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        StructuralFieldDeclaration {
                            id: structural_field_id(1),
                            identity: "sequence".to_owned(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                            )),
                        },
                        StructuralFieldDeclaration {
                            id: structural_field_id(2),
                            identity: "metadata".to_owned(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(structural_type_id(2)),
                        },
                    ],
                },
            },
            StructuralTypeDeclaration {
                id: structural_type_id(2),
                identity: "example::Root".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: structural_type_id(3),
                identity: "example::Worker".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        structural_domains: vec![StructuralDomainDeclaration {
            id: domain,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
            identity: "example::Occurrence::Pending".to_owned(),
            carrier: resource_type,
            content_projection: None,
        }],
        services: vec![ServiceDeclaration {
            id: service,
            identity: "example::DeviceIo".to_owned(),
            parents: Vec::new(),
        }],
        root_service_reach: terminal_psi::TerminalRootServiceReach {
            concrete: vec![service],
            installation_dependencies: Vec::new(),
        },
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            fixed_service_reach: Vec::new(),
            crash_routes: Vec::new(),
            id: boundary_machine_id(1),
            identity: "example::Occurrence::settle".to_owned(),
            attachment: Some(resource_type),
            scalar_parameters: Vec::new(),
            structural_parameters: vec![structural_parameter(place_id(30), 0, resource_type, true)],
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: vec![StructuralDomainRequirement {
                argument_index: 0,
                domain,
            }],
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: vec![service],
        }],
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(100),
                attachment: Some(structural_type_id(2)),
                parameters: Vec::new(),
                structural_parameters: vec![structural_parameter(
                    caller_place,
                    0,
                    resource_type,
                    false,
                )],
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: vec![StructuralPlaceDeclaration {
                    id: caller_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                }],
                entry_claims: vec![EntryClaim {
                    claim: claim_id(1),
                    input: caller_place,
                    path: Vec::new(),
                }],
                published_service_ceiling: vec![service],
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(100),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(100),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: operation_id(1),
                        result: OperationResult::Unit,
                        kind: OperationKind::CallUnit {
                            erased_arguments: Vec::new(),
                            arguments: Vec::new(),
                            callee: machine_id(101),
                            structural_arguments: vec![StructuralArgument {
                                place: caller_place,
                                access: StructuralAccess::Owned,
                                path: Vec::new(),
                            }],
                            claim_transfers: vec![ClaimTransfer {
                                claim: claim_id(1),
                                argument_index: 0,
                            }],
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                    }],
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(100),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: contract_id(100),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(101),
                attachment: Some(structural_type_id(3)),
                parameters: Vec::new(),
                structural_parameters: vec![structural_parameter(
                    callee_place,
                    0,
                    resource_type,
                    false,
                )],
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: vec![StructuralPlaceDeclaration {
                    id: callee_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                }],
                entry_claims: vec![EntryClaim {
                    claim: claim_id(1),
                    input: callee_place,
                    path: Vec::new(),
                }],
                published_service_ceiling: vec![service],
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(101),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(101),
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            id: operation_id(2),
                            result: OperationResult::Unit,
                            kind: OperationKind::PortWrite {
                                service,
                                port: 0x3f8,
                                value: 0x5a,
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            id: operation_id(3),
                            result: OperationResult::Unit,
                            kind: OperationKind::BoundaryCall {
                                boundary: boundary_machine_id(1),
                                arguments: Vec::new(),
                                structural_arguments: vec![StructuralArgument {
                                    place: callee_place,
                                    access: StructuralAccess::Owned,
                                    path: Vec::new(),
                                }],
                                completion_receipts: vec![CompletionReceipt {
                                    claim: claim_id(1),
                                    argument_index: 0,
                                }],
                            },
                        },
                    ],
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(101),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: contract_id(101),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
        ],
    }
}

fn structural_call_fixture() -> TerminalModule {
    let mut module = structural_effect_fixture();
    let result_type = structural_type_id(1);
    let result_domain = structural_domain_id(1);

    let caller = &mut module.machines[0];
    caller.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: place_id(12),
        structural_type: result_type,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: vec![result_domain],
        projected_qualifications: Vec::new(),
    });
    caller.structural_places.extend([
        StructuralPlaceDeclaration {
            id: place_id(11),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(1),
                structural_type: result_type,
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(12),
            kind: StructuralPlaceKind::Result,
        },
    ]);
    caller.blocks[0].operations[0].result =
        OperationResult::Structural(StructuralOperationResult {
            place: place_id(11),
            structural_type: result_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: vec![result_domain],
            projected_qualifications: Vec::new(),
            claims: vec![StructuralResultClaimBinding {
                claim: claim_id(1),
                path: Vec::new(),
            }],
        });
    caller.blocks[0].operations[0].kind = OperationKind::CallStructural {
        callee: machine_id(101),
        structural_arguments: vec![StructuralArgument {
            place: place_id(10),
            access: StructuralAccess::Owned,
            path: Vec::new(),
        }],
        claim_transfers: vec![ClaimTransfer {
            claim: claim_id(1),
            argument_index: 0,
        }],
        returned_claim_transfers: vec![StructuralResultClaimTransfer {
            callee_claim: claim_id(1),
            caller_claim: claim_id(1),
        }],
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        selected_evidence: Vec::new(),
    };
    caller.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(100),
        source: place_id(11),
        returned_claims: vec![claim_id(1)],
        trivial_affine_discards: Vec::new(),
    };

    let callee = &mut module.machines[1];
    callee.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: place_id(21),
        structural_type: result_type,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: vec![result_domain],
        projected_qualifications: Vec::new(),
    });
    callee.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(21),
        kind: StructuralPlaceKind::Result,
    });
    callee.blocks[0].operations.truncate(1);
    callee.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(101),
        source: place_id(20),
        returned_claims: vec![claim_id(1)],
        trivial_affine_discards: Vec::new(),
    };

    module
}

fn multi_claim_structural_call_fixture() -> TerminalModule {
    let mut module = structural_call_fixture();
    let element_type = structural_type_id(2);
    module.structural_types[0].shape = StructuralTypeShape::FixedArray {
        element: element_type,
        length: 2,
    };
    let paths = [
        vec![StructuralPathSegment::FixedIndex(0)],
        vec![StructuralPathSegment::FixedIndex(1)],
    ];

    for machine in &mut module.machines {
        machine.entry_claims = paths
            .iter()
            .enumerate()
            .map(|(index, path)| EntryClaim {
                claim: claim_id(index as u64 + 1),
                input: machine.structural_parameters[0].place,
                path: path.clone(),
            })
            .collect();
        let Terminator::ReturnStructural {
            returned_claims, ..
        } = &mut machine.blocks[0].terminator
        else {
            unreachable!()
        };
        *returned_claims = vec![claim_id(1), claim_id(2)];
    }

    let operation = &mut module.machines[0].blocks[0].operations[0];
    let OperationResult::Structural(result) = &mut operation.result else {
        unreachable!()
    };
    result.claims = paths
        .iter()
        .enumerate()
        .map(|(index, path)| StructuralResultClaimBinding {
            claim: claim_id(index as u64 + 1),
            path: path.clone(),
        })
        .collect();
    let OperationKind::CallStructural {
        claim_transfers,
        returned_claim_transfers,
        ..
    } = &mut operation.kind
    else {
        unreachable!()
    };
    *claim_transfers = vec![
        ClaimTransfer {
            claim: claim_id(1),
            argument_index: 0,
        },
        ClaimTransfer {
            claim: claim_id(2),
            argument_index: 0,
        },
    ];
    *returned_claim_transfers = vec![
        StructuralResultClaimTransfer {
            callee_claim: claim_id(1),
            caller_claim: claim_id(1),
        },
        StructuralResultClaimTransfer {
            callee_claim: claim_id(2),
            caller_claim: claim_id(2),
        },
    ];
    module
}

fn project_boundary_argument(
    module: &mut TerminalModule,
    path: Vec<StructuralPathSegment>,
    projected_type: StructuralTypeId,
) {
    let boundary_caller = module.machines.pop().expect("boundary caller fixture");
    module.machines = vec![boundary_caller];
    module.entry = module.machines[0].id;

    let boundary = &mut module.boundary_machines[0];
    boundary.requires.clear();
    boundary.attachment = Some(projected_type);
    boundary.structural_parameters[0].structural_type = projected_type;
    boundary.structural_parameters[0].qualifications.clear();
    module.machines[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    module.machines[0].structural_parameters[0]
        .qualifications
        .clear();
    let Terminator::ReturnUnit { edge, .. } = module.machines[0].blocks[0].terminator else {
        unreachable!()
    };
    module.machines[0].blocks[0].terminator = Terminator::Crash {
        edge,
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    module.machines[0].contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    project_boundary_path_only(module, path);
}

fn project_boundary_path_only(module: &mut TerminalModule, path: Vec<StructuralPathSegment>) {
    module.machines[0].entry_claims[0].path = path.clone();
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = path;
}

fn fixed_array_custody_fixture() -> TerminalModule {
    let mut module = structural_effect_fixture();
    let element = structural_type_id(1);
    let array = structural_type_id(4);
    module.structural_types.push(StructuralTypeDeclaration {
        id: array,
        identity: "example::OccurrencePair".to_owned(),
        shape: StructuralTypeShape::FixedArray { element, length: 2 },
    });

    let boundary_caller = module.machines.pop().expect("boundary caller fixture");
    module.machines = vec![boundary_caller];
    module.entry = module.machines[0].id;

    for boundary in &mut module.boundary_machines {
        boundary.requires.clear();
        for parameter in &mut boundary.structural_parameters {
            parameter.qualifications.clear();
        }
    }
    for machine in &mut module.machines {
        for parameter in &mut machine.structural_parameters {
            parameter.multiplicity = StructuralMultiplicity::Linear;
            parameter.qualifications.clear();
        }
    }

    module.machines[0].structural_parameters[0].structural_type = array;
    module.machines[0].entry_claims[0].path = vec![StructuralPathSegment::FixedIndex(0)];
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(0)];
    let caller_input = module.machines[0].structural_parameters[0].place;
    module.machines[0].entry_claims.push(EntryClaim {
        claim: claim_id(2),
        input: caller_input,
        path: vec![StructuralPathSegment::FixedIndex(1)],
    });
    let mut second_call = module.machines[0].blocks[0].operations[1].clone();
    second_call.id = operation_id(4);
    let OperationKind::BoundaryCall {
        structural_arguments,
        completion_receipts,
        ..
    } = &mut second_call.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(1)];
    completion_receipts[0].claim = claim_id(2);
    module.machines[0].blocks[0].operations.push(second_call);
    module
}

fn proof_recursive_component_fixture() -> TerminalProofRecursiveComponent {
    let contract = |raw| ContractId::new(raw).expect("nonzero proof contract identity");
    TerminalProofRecursiveComponent {
        ranking_relation: TerminalProofRankingRelation::StructuralSubterm,
        rank_type_identity: "package::Node".into(),
        types: vec![TerminalProofRecursiveType {
            identity: "package::Node".into(),
            fields: vec![
                TerminalProofRecursiveField {
                    identity: "package::Node::left".into(),
                    type_identity: "package::Node".into(),
                },
                TerminalProofRecursiveField {
                    identity: "package::Node::right".into(),
                    type_identity: "package::Node".into(),
                },
            ],
        }],
        members: vec![
            TerminalProofRecursiveMember {
                contract: contract(1001),
                machine_identity: "package::left".into(),
                rank_parameter_identity: "package::left::node".into(),
            },
            TerminalProofRecursiveMember {
                contract: contract(1002),
                machine_identity: "package::right".into(),
                rank_parameter_identity: "package::right::node".into(),
            },
        ],
        edges: vec![
            TerminalProofRecursiveEdge {
                caller: contract(1001),
                callee: contract(1002),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::left::step".into(),
                    statement_index: 0,
                },
                strict_member_path: vec!["package::Node::left".into()],
            },
            TerminalProofRecursiveEdge {
                caller: contract(1001),
                callee: contract(1002),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::left::step".into(),
                    statement_index: 1,
                },
                strict_member_path: vec!["package::Node::right".into()],
            },
            TerminalProofRecursiveEdge {
                caller: contract(1002),
                callee: contract(1001),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::right::step".into(),
                    statement_index: 0,
                },
                strict_member_path: vec!["package::Node::left".into()],
            },
            TerminalProofRecursiveEdge {
                caller: contract(1002),
                callee: contract(1001),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::right::step".into(),
                    statement_index: 1,
                },
                strict_member_path: vec!["package::Node::right".into()],
            },
        ],
    }
}

fn unit_fixture() -> TerminalModule {
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(900),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine_id(900),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(900),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: block_id(900),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: edge_id(900),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: contract_id(900),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn ranked_countdown_fixture() -> TerminalModule {
    let mut module = unit_fixture();
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let scalar = ScalarType::Integer(integer);
    let initial = value_id(901);
    let rank = value_id(902);
    let zero = value_id(903);
    let condition = value_id(904);
    let one = value_id(905);
    let next = value_id(906);
    let preheader = block_id(900);
    let header = block_id(901);
    let decrement = block_id(902);
    let done = block_id(903);
    let preheader_edge = edge_id(900);
    let guard_edge = edge_id(901);
    let exit_edge = edge_id(902);
    let backedge = edge_id(903);
    let return_edge = edge_id(904);
    let machine = &mut module.machines[0];
    machine.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: initial,
        scalar_type: scalar,
    }];
    machine.ranked_scc = Some(TerminalRankedScc::Natural(vec![
        terminal_psi::TerminalNaturalCycle {
            rank_type: integer,
            ranks: vec![
                terminal_psi::TerminalBlockNaturalRank {
                    block: header,
                    value: rank,
                },
                terminal_psi::TerminalBlockNaturalRank {
                    block: decrement,
                    value: rank,
                },
            ],
            edges: vec![
                terminal_psi::TerminalNaturalRankEdge {
                    edge: guard_edge,
                    source: header,
                    target: decrement,
                    successor_rank: rank,
                    comparison: terminal_psi::TerminalNaturalRankComparison::Preserving,
                },
                terminal_psi::TerminalNaturalRankEdge {
                    edge: backedge,
                    source: decrement,
                    target: header,
                    successor_rank: next,
                    comparison: terminal_psi::TerminalNaturalRankComparison::Strict,
                },
            ],
        },
    ]));
    machine.entry = preheader;
    machine.blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: preheader,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Jump {
                structural_arguments: Vec::new(),
                edge: preheader_edge,
                target: header,
                arguments: vec![initial],
                erased_arguments: Vec::new(),
                residual_affine_discards: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: header,
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: rank,
                scalar_type: scalar,
            }],
            operations: vec![
                Operation {
                    static_reach_binding: None,
                    id: operation_id(901),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: zero,
                        scalar_type: scalar,
                    }),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(0),
                    },
                },
                Operation {
                    static_reach_binding: None,
                    id: operation_id(902),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: condition,
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::IntegerLessThan {
                        left: zero,
                        right: rank,
                    },
                },
            ],
            terminator: Terminator::Conditional {
                condition,
                when_true: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: guard_edge,
                    target: decrement,
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: exit_edge,
                    target: done,
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: decrement,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    static_reach_binding: None,
                    id: operation_id(903),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: one,
                        scalar_type: scalar,
                    }),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(1),
                    },
                },
                Operation {
                    static_reach_binding: None,
                    id: operation_id(904),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: next,
                        scalar_type: scalar,
                    }),
                    kind: OperationKind::ExactIntegerSubtract {
                        left: rank,
                        right: one,
                        obligation: obligation_id(900),
                    },
                },
            ],
            terminator: Terminator::Jump {
                structural_arguments: Vec::new(),
                edge: backedge,
                target: header,
                arguments: vec![next],
                erased_arguments: Vec::new(),
                residual_affine_discards: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: done,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: return_edge,
                trivial_affine_discards: Vec::new(),
            },
        },
    ];
    module
}

fn fixture() -> TerminalModule {
    let integer = i32_type();
    let scalar_type = ScalarType::Integer(integer);
    let signed = |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).unwrap();
    let unsigned_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unsigned =
        |value| ScalarTerm::integer(unsigned_type, IntegerValue::Unsigned(value)).unwrap();

    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine_id(1),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(5),
                scalar_type: ScalarType::Boolean,
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(4),
                scalar_type,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: operation_id(1),
                        result: OperationResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: value_id(1),
                            scalar_type,
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(-7),
                        },
                    }],
                    terminator: Terminator::Jump {
                        structural_arguments: Vec::new(),
                        edge: edge_id(1),
                        target: block_id(2),
                        arguments: vec![value_id(1)],
                        erased_arguments: Vec::new(),
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(2),
                    parameters: vec![ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(2),
                        scalar_type,
                    }],
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: operation_id(2),
                        result: OperationResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: value_id(3),
                            scalar_type,
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(-7),
                        },
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(2),
                        value: value_id(3),
                    },
                },
            ],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: vec![
                    Proposition::Truth,
                    Proposition::Falsehood,
                    Proposition::Atom(proposition_id(1)),
                    Proposition::Equal(ScalarTerm::boolean(false), ScalarTerm::boolean(true)),
                    Proposition::LessThan(signed(-8), signed(-7)),
                    Proposition::LessOrEqual(unsigned(1), unsigned(2)),
                    Proposition::Conjunction(vec![Proposition::Truth, Proposition::Falsehood]),
                    Proposition::Implication {
                        premise: Box::new(Proposition::Truth),
                        conclusion: Box::new(Proposition::Atom(proposition_id(2))),
                    },
                ],
                ensures: vec![
                    ContractClause {
                        obligation: obligation_id(1),
                        proposition: Proposition::Equal(
                            ScalarTerm::value(value_id(4), scalar_type),
                            signed(-7),
                        ),
                    },
                    ContractClause {
                        obligation: obligation_id(2),
                        proposition: Proposition::Truth,
                    },
                ],
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn content_conservation_fixture(vocabulary_marker: VocabularyMarker) -> TerminalModule {
    let parameter_place = place_id(1);
    let result_place = place_id(2);
    let projection = ContentProjectionIdentity {
        domain: ContentDomainId::new(7).expect("domain"),
        projection_report_fingerprint: 0x1234,
    };
    let projected = |version, root, field: Option<&str>| ContentTerm::Projection {
        projection,
        subject: ContentStructuralPlace {
            version,
            root,
            segments: field
                .map(|field| vec![ContentPlaceSegment::Field(field.to_owned())])
                .unwrap_or_default(),
        },
    };
    let entry = projected(ContentPlaceVersion::Entry, parameter_place, None);
    let left = projected(ContentPlaceVersion::Current, result_place, Some("left"));
    let right = projected(ContentPlaceVersion::Current, result_place, Some("right"));
    let proposition = Proposition::ContentConservation(ContentConservation::new(
        ContentAlgebra {
            kind: ContentAlgebraKind::IntervalSet,
            parameter: "Address".to_owned(),
        },
        entry,
        ContentTerm::separate([right, left]).expect("canonical separation"),
    ));
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker,
        entry: machine_id(80),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine_id(80),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(80),
                scalar_type: ScalarType::Boolean,
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(81),
                scalar_type: ScalarType::Boolean,
            }),
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: parameter_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                },
                StructuralPlaceDeclaration {
                    id: result_place,
                    kind: StructuralPlaceKind::Result,
                },
            ],
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(80),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: block_id(80),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: edge_id(80),
                    value: value_id(80),
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: contract_id(80),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: vec![ContractClause {
                    obligation: obligation_id(80),
                    proposition,
                }],
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn identity_reshuffle_fixture(vocabulary_marker: VocabularyMarker) -> TerminalModule {
    let mut module = content_conservation_fixture(vocabulary_marker);
    let input_root = module.machines[0].structural_places[0].id;
    let output_root = module.machines[0].structural_places[1].id;
    module.machines[0].content_identity_reshuffles = vec![ContentIdentityReshuffle {
        claim: claim_id(1),
        input: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: input_root,
            segments: vec![ContentPlaceSegment::Field("payload".to_owned())],
        },
        output: ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: output_root,
            segments: vec![ContentPlaceSegment::Field("payload".to_owned())],
        },
        projections: vec![
            ClaimContentProjection {
                projection: ContentProjectionIdentity {
                    domain: ContentDomainId::new(7).expect("domain"),
                    projection_report_fingerprint: 0x1234,
                },
                algebra: ContentAlgebra {
                    kind: ContentAlgebraKind::IntervalSet,
                    parameter: "Address".to_owned(),
                },
            },
            ClaimContentProjection {
                projection: ContentProjectionIdentity {
                    domain: ContentDomainId::new(8).expect("domain"),
                    projection_report_fingerprint: 0x5678,
                },
                algebra: ContentAlgebra {
                    kind: ContentAlgebraKind::CountedQuantity,
                    parameter: "Byte".to_owned(),
                },
            },
        ],
    }];
    module
}

fn entry_claim_fixture() -> TerminalModule {
    let mut module = identity_reshuffle_fixture(VocabularyMarker::CURRENT);
    let reshuffle = module.machines[0].content_identity_reshuffles[0].clone();
    module.machines[0].content_entry_claims = vec![ContentEntryClaim {
        claim: reshuffle.claim,
        input: reshuffle.input,
        projections: reshuffle.projections,
    }];
    module
}

fn partition_composition_fixture() -> TerminalModule {
    let mut module = identity_reshuffle_fixture(VocabularyMarker::CURRENT);
    let machine = &mut module.machines[0];
    machine.content_identity_reshuffles[0]
        .projections
        .truncate(1);
    let projection = machine.content_identity_reshuffles[0].projections[0].projection;
    let algebra = machine.content_identity_reshuffles[0].projections[0]
        .algebra
        .clone();
    let result_root = machine.content_identity_reshuffles[0].output.root;
    let source_input_root = place_id(90);
    let source_result_root = place_id(91);
    let place = |version, root, field: Option<&str>| ContentStructuralPlace {
        version,
        root,
        segments: field
            .into_iter()
            .map(|field| ContentPlaceSegment::Field(field.to_owned()))
            .collect(),
    };
    let term = |subject| ContentTerm::Projection {
        projection,
        subject,
    };
    let source_input = place(ContentPlaceVersion::Entry, source_input_root, None);
    let source_left = place(
        ContentPlaceVersion::Current,
        source_result_root,
        Some("left"),
    );
    let source_right = place(
        ContentPlaceVersion::Current,
        source_result_root,
        Some("right"),
    );
    let target_input = machine.content_identity_reshuffles[0].input.clone();
    let target_left = place(ContentPlaceVersion::Current, result_root, Some("left"));
    let target_right = place(ContentPlaceVersion::Current, result_root, Some("right"));
    machine.content_identity_reshuffles[0].input = target_input.clone();
    machine.content_identity_reshuffles[0].output = target_left.clone();
    let source = ContentConservation::new(
        algebra.clone(),
        term(source_input.clone()),
        ContentTerm::separate([term(source_left.clone()), term(source_right.clone())])
            .expect("source partition"),
    );
    let derived = ContentConservation::new(
        algebra,
        term(target_input.clone()),
        ContentTerm::separate([term(target_left.clone()), term(target_right.clone())])
            .expect("derived partition"),
    );
    let mut substitutions = vec![
        ContentPlaceSubstitution {
            source: source_input,
            target: target_input,
        },
        ContentPlaceSubstitution {
            source: source_left,
            target: target_left,
        },
        ContentPlaceSubstitution {
            source: source_right,
            target: target_right,
        },
    ];
    substitutions.sort();
    machine.content_partition_compositions = vec![ContentPartitionComposition {
        producer_operation: operation_id(90),
        source_report_fingerprint: 0xfeed_face_dead_beef,
        source_structural_places: vec![
            StructuralPlaceDeclaration {
                id: source_input_root,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: source_result_root,
                kind: StructuralPlaceKind::Result,
            },
        ],
        source,
        input_claims: vec![claim_id(1)],
        substitutions,
        derived,
    }];
    module
}

fn call_fixture() -> TerminalModule {
    let boolean = |id| ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(id),
        scalar_type: ScalarType::Boolean,
    };
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(100),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(100),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(boolean(102)),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(100),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(100),
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            id: operation_id(100),
                            result: OperationResult::Scalar(boolean(100)),
                            kind: OperationKind::BooleanConstant { value: true },
                        },
                        Operation {
                            static_reach_binding: None,
                            id: operation_id(101),
                            result: OperationResult::Scalar(boolean(101)),
                            kind: OperationKind::Call {
                                erased_arguments: Vec::new(),
                                callee: machine_id(101),
                                arguments: vec![value_id(100)],
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                        },
                    ],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(100),
                        value: value_id(101),
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: contract_id(100),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(101),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![boolean(103)],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(boolean(104)),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(101),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(101),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(101),
                        value: value_id(103),
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: contract_id(101),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
        ],
    }
}

fn i32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).expect("i32")
}

macro_rules! id_constructor {
    ($function:ident, $type:ty) => {
        fn $function(raw: u64) -> $type {
            <$type>::new(raw).expect("test identities are nonzero")
        }
    };
}

id_constructor!(value_id, ValueId);
id_constructor!(machine_id, MachineId);
id_constructor!(block_id, BlockId);
id_constructor!(place_id, PlaceId);
id_constructor!(claim_id, ClaimId);
id_constructor!(operation_id, OperationId);
id_constructor!(suspension_crossing_id, SuspensionCrossingId);
id_constructor!(structural_type_id, StructuralTypeId);
id_constructor!(structural_field_id, StructuralFieldId);
id_constructor!(structural_case_id, StructuralCaseId);
id_constructor!(structural_domain_id, StructuralDomainId);
id_constructor!(service_id, ServiceId);
id_constructor!(boundary_machine_id, BoundaryMachineId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);
id_constructor!(proposition_id, PropositionId);
