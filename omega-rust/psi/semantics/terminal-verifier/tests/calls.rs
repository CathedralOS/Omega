//! Fixtures shared by the call tests: call, boundary and provider candidate
//! modules and the identity helpers.

#[path = "calls/boundary_crashes.rs"]
mod boundary_crashes;
#[path = "calls/byte_views.rs"]
mod byte_views;
#[path = "calls/call_publication_and_boundary_arguments.rs"]
mod call_publication_and_boundary_arguments;
#[path = "calls/declared_service_reach.rs"]
mod declared_service_reach;
#[path = "calls/erased_arguments.rs"]
mod erased_arguments;
#[path = "calls/provider_results.rs"]
mod provider_results;
#[path = "calls/scalar_and_structural_call_reconstruction.rs"]
mod scalar_and_structural_call_reconstruction;
#[path = "calls/scalar_array_arguments.rs"]
mod scalar_array_arguments;

use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType,
    MachineId, ObligationId, OperationId, PlaceId, Proposition, PropositionId, ScalarTerm,
    ScalarType, ServiceId, StructuralCaseId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, ContractClause, CrashCause, CrashRouteBucket,
    CrashRouteGuard, InstallationReachDependency, MachineContract, Operation, OperationKind,
    OperationResult, OutcomeSpecificEnsure, OutcomeSpecificGuard, ProviderCandidateConformance,
    ProviderRefinement, ProviderSignature, ServiceDeclaration, StructuralCaseDeclaration,
    StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{
    ModuleError, ObligationEvidence, ProofBundle, VerificationError,
    reconstruct_terminal_obligations, validate_module, verify_module,
};

fn boundary_call_module() -> TerminalModule {
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
        boundary_machines: vec![BoundaryMachineDeclaration {
            fixed_service_reach: Vec::new(),
            crash_routes: Vec::new(),
            id: boundary_id(1),
            identity: "test::observe".into(),
            attachment: None,
            scalar_parameters: vec![ScalarType::Boolean],
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
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
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine_id(1),
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
            entry: block_id(1),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: block_id(1),
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        static_reach_binding: None,
                        id: operation_id(1),
                        result: OperationResult::Scalar(boolean_declaration(value_id(1))),
                        kind: OperationKind::BooleanConstant { value: true },
                    },
                    Operation {
                        static_reach_binding: None,
                        id: operation_id(2),
                        result: OperationResult::Unit,
                        kind: OperationKind::BoundaryCall {
                            boundary: boundary_id(1),
                            arguments: vec![value_id(1)],
                            structural_arguments: Vec::new(),
                            completion_receipts: Vec::new(),
                        },
                    },
                ],
                terminator: Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: Vec::new(),
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
        }],
    }
}

fn boundary_arguments_mut(module: &mut TerminalModule) -> &mut Vec<ValueId> {
    let OperationKind::BoundaryCall { arguments, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    arguments
}

fn provider_candidate_module() -> TerminalModule {
    let mut module = boundary_call_module();
    let provider_type = semantic_vocabulary::StructuralTypeId::new(1).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: provider_type,
        identity: "test::Provider".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    module
        .provider_candidates
        .push(ProviderCandidateConformance {
            boundary: boundary_id(1),
            requirement_identity: "test::observe".into(),
            provider_identity: "test::Provider::observe".into(),
            candidate_identity: "test::provider_candidate".into(),
            candidate: machine_id(2),
            signature: ProviderSignature {
                parameters: Vec::new(),
            },
            refinement: ProviderRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        });
    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(2),
        attachment: Some(provider_type),
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![boolean_declaration(value_id(2))],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
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
    });
    module
}

fn call_module() -> TerminalModule {
    let caller_constant = value_id(1);
    let call_result = value_id(2);
    let caller_result = value_id(3);
    let callee_parameter = value_id(4);
    let callee_result = value_id(5);
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
        machines: vec![
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(1),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(boolean_declaration(caller_result)),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            id: operation_id(1),
                            result: terminal_psi::OperationResult::Scalar(boolean_declaration(
                                caller_constant,
                            )),
                            kind: OperationKind::BooleanConstant { value: true },
                        },
                        Operation {
                            static_reach_binding: None,
                            id: operation_id(2),
                            result: terminal_psi::OperationResult::Scalar(boolean_declaration(
                                call_result,
                            )),
                            kind: OperationKind::Call {
                                erased_arguments: Vec::new(),
                                callee: machine_id(2),
                                arguments: vec![caller_constant],
                                requirement_obligations: vec![obligation_id(1)],
                                crash_continuations: Vec::new(),
                            },
                        },
                    ],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(1),
                        value: call_result,
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
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![boolean_declaration(callee_parameter)],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(boolean_declaration(callee_result)),
                structural_places: Vec::new(),
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
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(2),
                        value: callee_parameter,
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: contract_id(2),
                    crash_routes: Vec::new(),
                    requires: vec![Proposition::Equal(
                        boolean_value(4),
                        ScalarTerm::boolean(true),
                    )],
                    ensures: vec![ContractClause {
                        obligation: obligation_id(2),
                        proposition: Proposition::Equal(boolean_value(5), boolean_value(4)),
                    }],
                    outcome_specific_ensures: Vec::new(),
                },
            },
        ],
    }
}

fn payloadless_guarded_call_module() -> TerminalModule {
    let result_type = structural_type_id(1);
    let success = structural_case_id(1);
    let failure = structural_case_id(2);
    let call_operation = operation_id(1);
    let constructor_operation = operation_id(2);
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: result_type,
            identity: "test::Outcome".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![
                    StructuralCaseDeclaration {
                        id: success,
                        identity: "Success".into(),
                        fields: Vec::new(),
                    },
                    StructuralCaseDeclaration {
                        id: failure,
                        identity: "Failure".into(),
                        fields: Vec::new(),
                    },
                ],
            },
        }],
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
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                    reference_sources: Vec::new(),
                    place: place_id(2),
                    structural_type: result_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }),
                structural_places: vec![
                    StructuralPlaceDeclaration {
                        id: place_id(1),
                        kind: StructuralPlaceKind::OperationResult {
                            producer: call_operation,
                            structural_type: result_type,
                        },
                    },
                    StructuralPlaceDeclaration {
                        id: place_id(2),
                        kind: StructuralPlaceKind::Result,
                    },
                ],
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
                        id: call_operation,
                        result: OperationResult::Structural(StructuralOperationResult {
                            place: place_id(1),
                            structural_type: result_type,
                            multiplicity: StructuralMultiplicity::Unrestricted,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                            claims: Vec::new(),
                        }),
                        kind: OperationKind::CallStructural {
                            callee: machine_id(2),
                            structural_arguments: Vec::new(),
                            claim_transfers: Vec::new(),
                            returned_claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                            selected_evidence: Vec::new(),
                        },
                    }],
                    terminator: Terminator::ReturnStructural {
                        edge: edge_id(1),
                        source: place_id(1),
                        returned_claims: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: contract_id(1),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: vec![ContractClause {
                        obligation: obligation_id(2),
                        proposition: Proposition::Truth,
                    }],
                    outcome_specific_ensures: Vec::new(),
                },
            },
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(2),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                    reference_sources: Vec::new(),
                    place: place_id(4),
                    structural_type: result_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }),
                structural_places: vec![
                    StructuralPlaceDeclaration {
                        id: place_id(3),
                        kind: StructuralPlaceKind::OperationResult {
                            producer: constructor_operation,
                            structural_type: result_type,
                        },
                    },
                    StructuralPlaceDeclaration {
                        id: place_id(4),
                        kind: StructuralPlaceKind::Result,
                    },
                ],
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(2),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(2),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: constructor_operation,
                        result: OperationResult::Structural(StructuralOperationResult {
                            place: place_id(3),
                            structural_type: result_type,
                            multiplicity: StructuralMultiplicity::Unrestricted,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                            claims: Vec::new(),
                        }),
                        kind: OperationKind::EstablishScalarCase {
                            result_case: success,
                            fields: vec![],
                        },
                    }],
                    terminator: Terminator::ReturnStructural {
                        edge: edge_id(2),
                        source: place_id(3),
                        returned_claims: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: contract_id(2),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: vec![
                        OutcomeSpecificEnsure {
                            guard: OutcomeSpecificGuard {
                                result_type,
                                result_case: success,
                            },
                            position: 0,
                            obligation: obligation_id(1),
                            proposition: Proposition::Truth,
                            evidence: None,
                        },
                        OutcomeSpecificEnsure {
                            guard: OutcomeSpecificGuard {
                                result_type,
                                result_case: failure,
                            },
                            position: 0,
                            obligation: obligation_id(3),
                            proposition: Proposition::Truth,
                            evidence: None,
                        },
                    ],
                },
            },
        ],
    }
}

fn call_kind_mut(
    module: &mut TerminalModule,
) -> (&mut MachineId, &mut Vec<ValueId>, &mut Vec<ObligationId>) {
    let OperationKind::Call {
        callee,
        arguments,
        requirement_obligations,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    (callee, arguments, requirement_obligations)
}

fn call_crash_continuations_mut(module: &mut TerminalModule) -> &mut Vec<CrashRouteBucket> {
    let OperationKind::Call {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    crash_continuations
}

fn semantic_axiom_evidence(
    obligation: ObligationId,
    proposition: Proposition,
    index: usize,
    identity: u64,
) -> ObligationEvidence {
    ObligationEvidence {
        obligation,
        route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: EvidenceIdentity::new(identity).unwrap(),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof: ProofNode {
                conclusion: proposition,
                rule: ProofRule::SemanticAxiom { index },
            },
        }),
    }
}

fn boolean_declaration(id: ValueId) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type: ScalarType::Boolean,
    }
}

fn boolean_value(raw: u64) -> ScalarTerm {
    ScalarTerm::value(value_id(raw), ScalarType::Boolean)
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
}

fn boundary_id(raw: u64) -> BoundaryMachineId {
    BoundaryMachineId::new(raw).unwrap()
}

fn block_id(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).unwrap()
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}

fn contract_id(raw: u64) -> ContractId {
    ContractId::new(raw).unwrap()
}

fn obligation_id(raw: u64) -> ObligationId {
    ObligationId::new(raw).unwrap()
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}

fn place_id(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}

fn structural_type_id(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}

fn structural_case_id(raw: u64) -> StructuralCaseId {
    StructuralCaseId::new(raw).unwrap()
}

fn service_id(raw: u64) -> ServiceId {
    ServiceId::new(raw).unwrap()
}
