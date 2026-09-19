//! Fixtures shared by the structural unit verification tests: root, nested
//! partial affine and nominal affine modules, structural parameters and
//! content predicates.

#[path = "structural_unit/borrowed_frontier.rs"]
mod borrowed_frontier;
#[path = "structural_unit/boundary_buffers.rs"]
mod boundary_buffers;
#[path = "structural_unit/disjoint_borrowed_projections.rs"]
mod disjoint_borrowed_projections;
#[path = "structural_unit/jumps_and_crash_routes.rs"]
mod jumps_and_crash_routes;
#[path = "structural_unit/nominal_affine_cleanup.rs"]
mod nominal_affine_cleanup;
#[path = "structural_unit/owned_subloans.rs"]
mod owned_subloans;
#[path = "structural_unit/parameter_returns.rs"]
mod parameter_returns;
#[path = "structural_unit/partial_affine_moves.rs"]
mod partial_affine_moves;
#[path = "structural_unit/primitive_snapshots.rs"]
mod primitive_snapshots;
#[path = "structural_unit/provider_attachments_and_cleanup_targets.rs"]
mod provider_attachments_and_cleanup_targets;
#[path = "structural_unit/reference_results.rs"]
mod reference_results;
#[path = "structural_unit/result_residuals.rs"]
mod result_residuals;
#[path = "structural_unit/write_only_attenuation.rs"]
mod write_only_attenuation;

use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, ContentAlgebra,
    ContentAlgebraKind, ContentDomainId, ContentPlaceSegment, ContentPlaceVersion,
    ContentProjectionExpression, ContentProjectionIdentity, ContentProjectionScalar,
    ContentStructuralPlace, ContentTerm, ContractId, EdgeId, EvidenceIdentity, IntegerSign,
    IntegerType, MachineId, ObligationId, OperationId, PlaceId, Proposition, ScalarTerm,
    ScalarType, ServiceId, StructuralDomainId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, ClaimContentProjection, ClaimTransfer, CompletionReceipt,
    ContentEntryClaim, ContractClause, EntryClaim, MachineContract, NominalAffineCleanup,
    Operation, OperationKind, OperationResult, ServiceDeclaration, StructuralAccess,
    StructuralAffineDiscard, StructuralArgument, StructuralContentProjection,
    StructuralDomainDeclaration, StructuralDomainRequirement, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{
    ModuleError, ObligationEvidence, ProofBundle, reconstruct_structural_ownership_frontiers,
    validate_module, verify_module,
};

fn provider_field_id() -> semantic_vocabulary::StructuralFieldId {
    semantic_vocabulary::StructuralFieldId::new(1).unwrap()
}

fn unused_provider_attachment_module() -> TerminalModule {
    let mut module = nominal_affine_module();
    module.machines.remove(0);
    module.entry = machine_id(2);
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            id: provider_field_id(),
            identity: "console".into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: StructuralFieldType::Erased {
                type_identity: "named(name(example::ConsoleProvider))".into(),
            },
        }],
    };
    module.boundary_machines.push(BoundaryMachineDeclaration {
        fixed_service_reach: Vec::new(),
        crash_routes: Vec::new(),
        id: boundary_id(1),
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
            field: provider_field_id(),
            boundary: boundary_id(1),
        },
    }
}

fn provider_boundary_call() -> Operation {
    Operation {
        static_reach_binding: None,
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: boundary_id(1),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    }
}

fn projected_boundary_qualification_module() -> TerminalModule {
    let mut module = write_only_primitive_store_module();
    let leaf = StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "QualifiedLeaf".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    let root = StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "QualifiedRoot".into(),
        shape: StructuralTypeShape::Record {
            fields: ["left", "right"]
                .into_iter()
                .enumerate()
                .map(|(index, identity)| StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(index as u64 + 1).unwrap(),
                    identity: identity.into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(leaf.id),
                })
                .collect(),
        },
    };
    let domain = StructuralDomainDeclaration {
        id: domain_id(1),
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
        identity: "QualifiedLeaf::Ready".into(),
        carrier: leaf.id,
        content_projection: None,
    };
    module.structural_types = vec![leaf.clone(), root.clone()];
    module.structural_domains = vec![domain.clone()];
    module.boundary_machines = vec![BoundaryMachineDeclaration {
        fixed_service_reach: Vec::new(),
        crash_routes: Vec::new(),
        id: boundary_id(1),
        identity: "consume_ready_leaf".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: place_id(2),
            position: 0,
            is_self: false,
            structural_type: leaf.id,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: vec![StructuralDomainRequirement {
            argument_index: 0,
            domain: domain.id,
        }],
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    }];
    let machine = &mut module.machines[0];
    machine.parameters.clear();
    machine.structural_parameters[0].structural_type = root.id;
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    machine.structural_parameters[0].access = StructuralAccess::Owned;
    machine.structural_parameters[0].projected_qualifications =
        vec![terminal_psi::StructuralPathQualification {
            path: vec![StructuralPathSegment::Field("left".into())],
            domain: domain.id,
        }];
    machine.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: boundary_id(1),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place_id(1),
                path: vec![StructuralPathSegment::Field("left".into())],
                access: StructuralAccess::SharedBorrow,
            }],
            completion_receipts: Vec::new(),
        },
    }];
    machine.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(1),
        trivial_affine_discards: vec![place_id(1)],
    };
    module
}

fn signed_i8() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).expect("i8"))
}

fn write_only_primitive_store_module() -> TerminalModule {
    let structural_type = StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "WriteOnlyI8".into(),
        shape: StructuralTypeShape::PrimitiveScalar(signed_i8()),
    };
    let parameter = StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: false,
        structural_type: structural_type.id,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::WriteOnlyBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let destination = parameter.place;
    let store = |raw| Operation {
        static_reach_binding: None,
        id: operation_id(raw),
        result: OperationResult::Unit,
        kind: OperationKind::WriteOnlyPrimitiveStore {
            destination,
            path: Vec::new(),
            value: value_id(1),
        },
    };
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(1),
        attachment: None,
        parameters: vec![ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(1),
            scalar_type: signed_i8(),
        }],
        structural_parameters: vec![parameter],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: place_id(1),
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
            operations: vec![store(1), store(2)],
            terminator: Terminator::ReturnUnit {
                edge: edge_id(1),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(1)),
    };
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine.id,
        structural_types: vec![structural_type],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: terminal_psi::TerminalRootServiceReach {
            concrete: Vec::new(),
            installation_dependencies: Vec::new(),
        },
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
        machines: vec![machine],
    }
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
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
        identity: "Pending".into(),
        carrier: resource.id,
        content_projection: Some(content_owner_projection()),
    };
    let port_io = ServiceDeclaration {
        id: service_id(1),
        identity: "PortIo".into(),
        parents: Vec::new(),
    };
    let mut boundary_parameter = structural_parameter(place_id(9));
    boundary_parameter.qualifications.clear();
    let boundary = BoundaryMachineDeclaration {
        fixed_service_reach: Vec::new(),
        crash_routes: Vec::new(),
        id: boundary_id(1),
        identity: "settle_port".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: vec![boundary_parameter],
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: vec![StructuralDomainRequirement {
            argument_index: 0,
            domain: pending.id,
        }],
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: vec![port_io.id],
    };

    let caller = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![structural_parameter(place_id(1))],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![structural_place(place_id(1))],
        entry_claims: vec![EntryClaim {
            claim: claim_id(1),
            input: place_id(1),
            path: Vec::new(),
        }],
        published_service_ceiling: vec![port_io.id],
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
                        place: place_id(1),
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
                edge: edge_id(1),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(1)),
    };

    let callee = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(2),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![structural_parameter(place_id(2))],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![structural_place(place_id(2))],
        entry_claims: vec![EntryClaim {
            claim: claim_id(1),
            input: place_id(2),
            path: Vec::new(),
        }],
        published_service_ceiling: vec![port_io.id],
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(2),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(2),
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    static_reach_binding: None,
                    id: operation_id(2),
                    result: OperationResult::Unit,
                    kind: OperationKind::PortWrite {
                        service: port_io.id,
                        port: 0x3f8,
                        value: b'X',
                    },
                },
                Operation {
                    static_reach_binding: None,
                    id: operation_id(3),
                    result: OperationResult::Unit,
                    kind: OperationKind::BoundaryCall {
                        boundary: boundary.id,
                        arguments: Vec::new(),
                        structural_arguments: vec![StructuralArgument {
                            place: place_id(2),
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
                edge: edge_id(2),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(2)),
    };

    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller.id,
        structural_types: vec![resource, other],
        structural_domains: vec![pending],
        services: vec![port_io],
        root_service_reach: terminal_psi::TerminalRootServiceReach {
            concrete: vec![service_id(1)],
            installation_dependencies: Vec::new(),
        },
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: vec![boundary],
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
        machines: vec![caller, callee],
    }
}

fn projected_unit_call_module() -> TerminalModule {
    let mut module = hard_root_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "[PortResource;1]".into(),
        shape: StructuralTypeShape::FixedArray {
            element: structural_type_id(1),
            length: 1,
        },
    });
    module.boundary_machines[0].requires.clear();
    module.machines[0].structural_parameters[0].structural_type = structural_type_id(3);
    module.machines[0].structural_parameters[0]
        .qualifications
        .clear();
    module.machines[0].entry_claims[0].path =
        vec![terminal_psi::StructuralPathSegment::FixedIndex(0)];
    module.machines[1].structural_parameters[0]
        .qualifications
        .clear();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![terminal_psi::StructuralPathSegment::FixedIndex(0)];
    module
}

fn two_element_projected_unit_call_module() -> TerminalModule {
    let mut module = projected_unit_call_module();
    let StructuralTypeShape::FixedArray { length, .. } = &mut module.structural_types[2].shape
    else {
        unreachable!()
    };
    *length = 2;
    module.machines[0].entry_claims.push(EntryClaim {
        claim: claim_id(2),
        input: place_id(1),
        path: vec![StructuralPathSegment::FixedIndex(1)],
    });
    module
}

fn partial_affine_field_module() -> TerminalModule {
    let token = StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    let pair = StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "Pair".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(1).expect("field identity"),
                    identity: "left".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(token.id),
                },
                StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(2).expect("field identity"),
                    identity: "middle".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(token.id),
                },
                StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(3).expect("field identity"),
                    identity: "right".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(token.id),
                },
            ],
        },
    };
    let other = StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "Other".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    let caller_parameter = StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: false,
        structural_type: pair.id,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let callee_parameter = StructuralParameterDeclaration {
        place: place_id(2),
        position: 0,
        is_self: false,
        structural_type: token.id,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let caller = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![caller_parameter],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![structural_place(place_id(1))],
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
                        place: place_id(1),
                        access: StructuralAccess::Owned,
                        path: vec![StructuralPathSegment::Field("right".into())],
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnitPartialAffine {
                edge: edge_id(1),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: vec![
                    StructuralAffineDiscard {
                        place: place_id(1),
                        path: vec![StructuralPathSegment::Field("middle".into())],
                        structural_type: token.id,
                    },
                    StructuralAffineDiscard {
                        place: place_id(1),
                        path: vec![StructuralPathSegment::Field("left".into())],
                        structural_type: token.id,
                    },
                ],
            },
        }],
        contract: empty_contract(contract_id(1)),
    };
    let callee = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(2),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![callee_parameter],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![structural_place(place_id(2))],
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
                trivial_affine_discards: vec![place_id(2)],
            },
        }],
        contract: empty_contract(contract_id(2)),
    };
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller.id,
        structural_types: vec![token, pair, other],
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
        machines: vec![caller, callee],
    }
}

fn multiple_move_partial_affine_field_module() -> TerminalModule {
    let mut module = partial_affine_field_module();
    let caller_block = &mut module.machines[0].blocks[0];
    let mut second_call = caller_block.operations[0].clone();
    second_call.id = operation_id(2);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut second_call.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("middle".into())];
    caller_block.operations.push(second_call);
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut caller_block.terminator
    else {
        unreachable!()
    };
    residual_affine_discards.remove(0);
    module
}

fn nested_partial_affine_field_module() -> TerminalModule {
    let mut module = partial_affine_field_module();
    let inner = StructuralTypeDeclaration {
        id: structural_type_id(4),
        identity: "Nested".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(4).expect("field identity"),
                    identity: "left".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(1)),
                },
                StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(5).expect("field identity"),
                    identity: "middle".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(1)),
                },
                StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(6).expect("field identity"),
                    identity: "right".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(1)),
                },
            ],
        },
    };
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        unreachable!()
    };
    fields[1].identity = "nested".into();
    fields[1].field_type = StructuralFieldType::Structural(inner.id);
    module.structural_types.push(inner);

    let caller_block = &mut module.machines[0].blocks[0];
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut caller_block.operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![
        StructuralPathSegment::Field("nested".into()),
        StructuralPathSegment::Field("middle".into()),
    ];
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut caller_block.terminator
    else {
        unreachable!()
    };
    *residual_affine_discards = vec![
        StructuralAffineDiscard {
            place: place_id(1),
            path: vec![StructuralPathSegment::Field("right".into())],
            structural_type: structural_type_id(1),
        },
        StructuralAffineDiscard {
            place: place_id(1),
            path: vec![
                StructuralPathSegment::Field("nested".into()),
                StructuralPathSegment::Field("right".into()),
            ],
            structural_type: structural_type_id(1),
        },
        StructuralAffineDiscard {
            place: place_id(1),
            path: vec![
                StructuralPathSegment::Field("nested".into()),
                StructuralPathSegment::Field("left".into()),
            ],
            structural_type: structural_type_id(1),
        },
        StructuralAffineDiscard {
            place: place_id(1),
            path: vec![StructuralPathSegment::Field("left".into())],
            structural_type: structural_type_id(1),
        },
    ];
    module
}

fn multiple_nested_partial_affine_field_module() -> TerminalModule {
    let mut module = nested_partial_affine_field_module();
    let caller_block = &mut module.machines[0].blocks[0];
    let mut second_call = caller_block.operations[0].clone();
    second_call.id = operation_id(2);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut second_call.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![
        StructuralPathSegment::Field("nested".into()),
        StructuralPathSegment::Field("left".into()),
    ];
    caller_block.operations.push(second_call);
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut caller_block.terminator
    else {
        unreachable!()
    };
    residual_affine_discards.remove(2);
    module
}

fn mixed_direct_nested_partial_affine_field_module() -> TerminalModule {
    let mut module = nested_partial_affine_field_module();
    let caller_block = &mut module.machines[0].blocks[0];
    let mut direct_call = caller_block.operations[0].clone();
    direct_call.id = operation_id(2);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut direct_call.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("right".into())];
    caller_block.operations.push(direct_call);
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut caller_block.terminator
    else {
        unreachable!()
    };
    residual_affine_discards.remove(0);
    module
}

fn nominal_affine_module() -> TerminalModule {
    let token = StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    let caller = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: place_id(1),
            position: 0,
            is_self: false,
            structural_type: token.id,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![structural_place(place_id(1))],
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
                    place: place_id(1),
                    structural_type: token.id,
                    cleanup_machine: machine_id(2),
                    cleanup_receiver: None,
                    requirement_obligations: Vec::new(),
                }],
            },
        }],
        contract: empty_contract(contract_id(1)),
    };
    let cleanup = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(2),
        attachment: Some(token.id),
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
        contract: empty_contract(contract_id(2)),
    };
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller.id,
        structural_types: vec![token],
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
        machines: vec![caller, cleanup],
    }
}

fn contextual_nominal_affine_module() -> TerminalModule {
    let mut module = nominal_affine_module();
    let field = semantic_vocabulary::StructuralFieldId::new(1).expect("field");
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            id: field,
            identity: "ready".into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
        }],
    };
    let caller_requirement = Proposition::Equal(
        ScalarTerm::boolean(true),
        ScalarTerm::boolean_field(place_id(1), field),
    );
    module.machines[0]
        .contract
        .requires
        .push(caller_requirement);
    let receiver = place_id(99);
    module.machines[1]
        .contract
        .requires
        .push(Proposition::Equal(
            ScalarTerm::boolean(true),
            ScalarTerm::boolean_field(receiver, field),
        ));
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].cleanup_receiver = Some(receiver);
    cleanups[0].requirement_obligations = vec![obligation_id(1)];
    module
}

fn two_requirement_contextual_nominal_affine_module() -> TerminalModule {
    let mut module = nominal_affine_module();
    let receiver = place_id(99);
    let fields = (1_u64..=3)
        .map(|identity| StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(identity).expect("field"),
            identity: format!("flag_{identity}"),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
        })
        .collect::<Vec<_>>();
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: fields.clone(),
    };
    module.machines[0].contract.requires = fields
        .iter()
        .map(|field| {
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(place_id(1), field.id),
            )
        })
        .collect();
    module.machines[1].contract.requires = fields[..2]
        .iter()
        .map(|field| {
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(receiver, field.id),
            )
        })
        .collect();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].cleanup_receiver = Some(receiver);
    cleanups[0].requirement_obligations = vec![obligation_id(1), obligation_id(2)];
    module
}

fn two_root_shared_contextual_nominal_affine_module() -> TerminalModule {
    let mut module = two_requirement_contextual_nominal_affine_module();
    let first = semantic_vocabulary::StructuralFieldId::new(1).expect("first field");
    let second = semantic_vocabulary::StructuralFieldId::new(2).expect("second field");
    let caller = &mut module.machines[0];
    caller
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
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(2),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    caller
        .contract
        .requires
        .extend([first, second].map(|field| {
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(place_id(2), field),
            )
        }));
    caller.contract.requires.sort();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &mut caller.blocks[0].terminator
    else {
        unreachable!()
    };
    let mut second_cleanup = cleanups[0].clone();
    second_cleanup.place = place_id(2);
    second_cleanup.requirement_obligations = vec![obligation_id(3), obligation_id(4)];
    cleanups.insert(0, second_cleanup);
    module
}

fn two_root_distinct_contextual_nominal_affine_module() -> TerminalModule {
    let mut module = two_root_shared_contextual_nominal_affine_module();
    let second_type = structural_type_id(2);
    let mut type_declaration = module.structural_types[0].clone();
    type_declaration.id = second_type;
    type_declaration.identity = "SecondToken".into();
    module.structural_types.push(type_declaration);

    let second_target_id = machine_id(3);
    let second_receiver = place_id(100);
    let mut second_target = module.machines[1].clone();
    second_target.id = second_target_id;
    second_target.attachment = Some(second_type);
    second_target.entry = block_id(3);
    second_target.blocks[0].id = block_id(3);
    second_target.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(3),
        trivial_affine_discards: Vec::new(),
    };
    second_target.contract.id = contract_id(3);
    for requirement in &mut second_target.contract.requires {
        let Proposition::Equal(_, ScalarTerm::BooleanField { root, .. }) = requirement else {
            unreachable!()
        };
        *root = second_receiver;
    }
    module.machines.push(second_target);

    let caller = &mut module.machines[0];
    caller.structural_parameters[1].structural_type = second_type;
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &mut caller.blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].structural_type = second_type;
    cleanups[0].cleanup_machine = second_target_id;
    cleanups[0].cleanup_receiver = Some(second_receiver);
    module
}

fn two_root_nominal_affine_module() -> TerminalModule {
    let mut module = nominal_affine_module();
    let caller = &mut module.machines[0];
    caller
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
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(2),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &mut caller.blocks[0].terminator
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

fn five_root_nominal_affine_module() -> TerminalModule {
    let mut module = two_root_nominal_affine_module();
    let caller = &mut module.machines[0];
    for position in 2_u32..5 {
        let place = place_id(u64::from(position + 1));
        caller
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
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::Parameter {
                position,
                is_self: false,
            },
        });
        let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &mut caller.blocks[0].terminator
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

fn two_root_one_executable_nominal_affine_module() -> TerminalModule {
    let mut module = two_root_nominal_affine_module();
    let mut second_cleanup = module.machines[1].clone();
    second_cleanup.id = machine_id(3);
    second_cleanup.entry = block_id(3);
    second_cleanup.blocks[0].id = block_id(3);
    second_cleanup.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(3),
        trivial_affine_discards: Vec::new(),
    };
    second_cleanup.contract.id = contract_id(3);
    let mut helper = module.machines[1].clone();
    helper.id = machine_id(4);
    helper.entry = block_id(4);
    helper.blocks[0].id = block_id(4);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(4),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(4);
    module.machines[1].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            arguments: Vec::new(),
            callee: helper.id,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].cleanup_machine = second_cleanup.id;
    module.machines.push(second_cleanup);
    module.machines.push(helper);
    module
}

fn executable_nominal_affine_module() -> TerminalModule {
    let mut module = nominal_affine_module();
    let helper_type = StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "Helper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            arguments: Vec::new(),
            callee: machine_id(3),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(3),
        attachment: Some(helper_type.id),
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
        entry: block_id(3),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(3),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(3),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(3)),
    });
    module
}

fn two_call_executable_nominal_affine_module() -> TerminalModule {
    let mut module = executable_nominal_affine_module();
    let helper_type = StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "SecondHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(2),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            arguments: Vec::new(),
            callee: machine_id(4),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    let mut second_helper = module.machines[2].clone();
    second_helper.id = machine_id(4);
    second_helper.attachment = Some(helper_type.id);
    second_helper.entry = block_id(4);
    second_helper.blocks[0].id = block_id(4);
    second_helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(4),
        trivial_affine_discards: Vec::new(),
    };
    second_helper.contract.id = contract_id(4);
    module.machines.push(second_helper);
    module
}

fn three_call_executable_nominal_affine_module() -> TerminalModule {
    let mut module = two_call_executable_nominal_affine_module();
    let helper_type = StructuralTypeDeclaration {
        id: structural_type_id(4),
        identity: "ThirdHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(3),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            arguments: Vec::new(),
            callee: machine_id(5),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    let mut third_helper = module.machines[2].clone();
    third_helper.id = machine_id(5);
    third_helper.attachment = Some(helper_type.id);
    third_helper.entry = block_id(5);
    third_helper.blocks[0].id = block_id(5);
    third_helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(5),
        trivial_affine_discards: Vec::new(),
    };
    third_helper.contract.id = contract_id(5);
    module.machines.push(third_helper);
    module
}

fn five_call_executable_nominal_affine_module() -> TerminalModule {
    let mut module = three_call_executable_nominal_affine_module();
    for raw in 6_u64..=7 {
        let mut helper = module.machines[2].clone();
        helper.id = machine_id(raw);
        helper.entry = block_id(raw);
        helper.blocks[0].id = block_id(raw);
        helper.blocks[0].terminator = Terminator::ReturnUnit {
            edge: edge_id(raw),
            trivial_affine_discards: Vec::new(),
        };
        helper.contract.id = contract_id(raw);
        module.machines[1].blocks[0].operations.push(Operation {
            static_reach_binding: None,
            id: operation_id(raw - 2),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                erased_arguments: Vec::new(),
                arguments: Vec::new(),
                callee: helper.id,
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
        module.machines.push(helper);
    }
    module
}

fn structural_parameter(place: PlaceId) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: vec![domain_id(1)],
        projected_qualifications: Vec::new(),
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
            projection: content_owner_projection().identity,
            algebra: ContentAlgebra {
                kind: ContentAlgebraKind::CountedQuantity,
                parameter: "Acknowledgement".to_owned(),
            },
        }],
    }
}

fn content_owner_projection() -> StructuralContentProjection {
    let algebra = ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "Acknowledgement".to_owned(),
    };
    let expression = ContentProjectionExpression::CountedQuantity(
        ContentProjectionScalar::Natural("1".to_owned()),
    );
    StructuralContentProjection {
        identity: ContentProjectionIdentity {
            domain: ContentDomainId::new(1).expect("content domain"),
            projection_report_fingerprint:
                language_semantics::content::terminal_projection_report_fingerprint(
                    &algebra,
                    &expression,
                ),
        },
        algebra,
        expression,
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
        erased_scalar_formals: Vec::new(),
        id,
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

fn content_predicate(root: PlaceId) -> Proposition {
    let projection = ContentProjectionIdentity {
        domain: ContentDomainId::new(1).expect("content domain"),
        projection_report_fingerprint: 1,
    };
    let projected = |field: &str| ContentTerm::Projection {
        projection,
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root,
            segments: vec![ContentPlaceSegment::Field(field.into())],
        },
    };
    Proposition::ContentConservation(semantic_vocabulary::ContentConservation::new(
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

fn boundary_call_mut(module: &mut TerminalModule) -> &mut Vec<CompletionReceipt> {
    let OperationKind::BoundaryCall {
        completion_receipts,
        ..
    } = &mut module.machines[1].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    completion_receipts
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
id_fn!(value_id, ValueId);
id_fn!(service_id, ServiceId);
id_fn!(structural_type_id, StructuralTypeId);
id_fn!(domain_id, StructuralDomainId);
