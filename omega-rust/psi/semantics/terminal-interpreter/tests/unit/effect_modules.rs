//! Effect, boundary, payloadless, unit and FMA module fixtures.

use super::call_modules::write_only_primitive_call_module;
use super::{
    block_id, boundary_id, claim_id, contract_id, edge_id, empty_contract, machine_id,
    operation_id, place_id, service_id, structural_case_id, structural_domain_id,
    structural_parameter, structural_place, structural_type_id, value_id,
};
use semantic_vocabulary::{IeeeFloatValue, ScalarType};
use terminal_codec::{encode_module, encode_proof_section};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, ByteSequenceCarrier, ClaimTransfer, CompletionReceipt,
    EntryClaim, MachineContract, Operation, OperationKind, OperationResult, ServiceDeclaration,
    StructuralAccess, StructuralArgument, StructuralDomainDeclaration, StructuralDomainRequirement,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::ProofBundle;

pub(super) fn reference_release_module() -> TerminalModule {
    let mut module = write_only_primitive_call_module();
    module.machines.truncate(1);
    let reference_type = structural_type_id(94);
    module.structural_types.push(StructuralTypeDeclaration {
        id: reference_type,
        identity: "test::MutableU8Reference".into(),
        shape: StructuralTypeShape::Reference {
            referent: structural_type_id(91),
            access: StructuralAccess::MutableBorrow,
        },
    });
    let caller = &mut module.machines[0];
    caller.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(94),
        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer: operation_id(94),
            structural_type: reference_type,
        },
    });
    caller.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: operation_id(94),
            result: OperationResult::Structural(StructuralOperationResult {
                place: place_id(94),
                structural_type: reference_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishReference {
                source: StructuralArgument {
                    place: place_id(91),
                    path: Vec::new(),
                    access: StructuralAccess::MutableBorrow,
                },
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(95),
            result: OperationResult::Unit,
            kind: OperationKind::ReleaseReference {
                source: place_id(94),
            },
        },
    ];
    module
}

pub(super) fn byte_sequence_literal_module(bytes: Vec<u8>) -> TerminalModule {
    let structural_type = structural_type_id(1);
    let literal = place_id(1);
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::BorrowedBytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        }],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            fixed_service_reach: Vec::new(),
            id: boundary_id(1),
            identity: "test::write_line".into(),
            attachment: None,
            parameter_order: vec![terminal_psi::BoundaryParameterKind::Structural],
            scalar_parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: place_id(2),
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: Vec::new(),
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
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: vec![StructuralPlaceDeclaration {
                id: literal,
                kind: semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal: 0,
                    structural_type,
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
                operations: vec![
                    Operation {
                        static_reach_binding: None,
                        id: operation_id(1),
                        result: OperationResult::Unit,
                        kind: OperationKind::EstablishByteSequenceLiteral {
                            destination: literal,
                            bytes,
                        },
                    },
                    Operation {
                        static_reach_binding: None,
                        id: operation_id(2),
                        result: OperationResult::Unit,
                        kind: OperationKind::BoundaryCall {
                            boundary: boundary_id(1),
                            arguments: Vec::new(),
                            structural_arguments: vec![StructuralArgument {
                                place: literal,
                                access: StructuralAccess::SharedBorrow,
                                path: Vec::new(),
                            }],
                            completion_receipts: Vec::new(),
                        },
                    },
                ],
                terminator: Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: empty_contract(contract_id(1)),
        }],
    }
}

pub(super) fn effect_artifact_sections() -> (Vec<u8>, Vec<u8>) {
    (
        encode_module(&effect_module()).expect("effect semantics encode"),
        encode_proof_section(&effect_module(), &ProofBundle::default())
            .expect("empty proof encodes"),
    )
}

pub(super) fn scalar_boundary_effect_module() -> TerminalModule {
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
            id: boundary_id(1),
            identity: "test::observe".into(),
            attachment: None,
            parameter_order: vec![
                terminal_psi::BoundaryParameterKind::Scalar,
                terminal_psi::BoundaryParameterKind::Scalar,
            ],
            scalar_parameters: vec![ScalarType::Boolean, ScalarType::Boolean],
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: Vec::new(),
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
                        result: OperationResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: value_id(1),
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: OperationKind::BooleanConstant { value: true },
                    },
                    Operation {
                        static_reach_binding: None,
                        id: operation_id(2),
                        result: OperationResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: value_id(2),
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: OperationKind::BooleanConstant { value: false },
                    },
                    Operation {
                        static_reach_binding: None,
                        id: operation_id(3),
                        result: OperationResult::Unit,
                        kind: OperationKind::BoundaryCall {
                            boundary: boundary_id(1),
                            arguments: vec![value_id(1), value_id(2)],
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
            contract: empty_contract(contract_id(1)),
        }],
    }
}

pub(super) fn structural_boundary_effect_module() -> TerminalModule {
    let mut module = scalar_boundary_effect_module();
    let structural_type = structural_type_id(1);
    let place = place_id(1);
    let producer = operation_id(3);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::ByteRead".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    module.boundary_machines[0].result = terminal_psi::BoundaryMachineResult::Structural(
        terminal_psi::BoundaryStructuralResultDeclaration {
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        },
    );
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place,
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            },
        });
    module.machines[0].blocks[0].operations[2].result =
        OperationResult::Structural(StructuralOperationResult {
            place,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        });
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!("scalar boundary fixture returns Unit")
    };
    trivial_affine_discards.push(place);
    module
}

pub(super) fn effect_module() -> TerminalModule {
    let structural_type = structural_type_id(1);
    let domain = structural_domain_id(1);
    let service = service_id(1);
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::Device".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: domain,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1)
                .expect("semantic domain identity"),
            identity: "test::Ready".into(),
            carrier: structural_type,
            content_projection: None,
        }],
        services: vec![ServiceDeclaration {
            id: service,
            identity: "test::PortIo".into(),
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
            id: boundary_id(1),
            identity: "test::acknowledge".into(),
            attachment: Some(structural_type),
            parameter_order: vec![terminal_psi::BoundaryParameterKind::Structural],
            scalar_parameters: Vec::new(),
            structural_parameters: vec![structural_parameter(place_id(3), structural_type, domain)],
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: vec![StructuralDomainRequirement {
                argument_index: 0,
                domain,
            }],
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: Vec::new(),
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
                id: machine_id(1),
                attachment: Some(structural_type),
                parameters: Vec::new(),
                structural_parameters: vec![structural_parameter(
                    place_id(1),
                    structural_type,
                    domain,
                )],
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: vec![structural_place(place_id(1))],
                entry_claims: vec![EntryClaim {
                    claim: claim_id(1),
                    input: place_id(1),
                    path: Vec::new(),
                }],
                published_service_ceiling: vec![service],
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
                        },
                        Operation {
                            static_reach_binding: None,
                            id: operation_id(2),
                            result: OperationResult::Unit,
                            kind: OperationKind::PortWrite {
                                service,
                                port: 0x20,
                                value: 0x20,
                            },
                        },
                    ],
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(1),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: empty_contract(contract_id(1)),
            },
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(2),
                attachment: Some(structural_type),
                parameters: Vec::new(),
                structural_parameters: vec![structural_parameter(
                    place_id(2),
                    structural_type,
                    domain,
                )],
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: vec![structural_place(place_id(2))],
                entry_claims: vec![EntryClaim {
                    claim: claim_id(1),
                    input: place_id(2),
                    path: Vec::new(),
                }],
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
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: operation_id(3),
                        result: OperationResult::Unit,
                        kind: OperationKind::BoundaryCall {
                            boundary: boundary_id(1),
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
                    }],
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(2),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: empty_contract(contract_id(2)),
            },
        ],
    }
}

pub(super) fn payloadless_case_module() -> TerminalModule {
    let structural_type = structural_type_id(1);
    let result_case = structural_case_id(1);
    let operation_place = place_id(1);
    let result_place = place_id(2);
    let mut module = unit_module();
    module.structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::Outcome".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![terminal_psi::StructuralCaseDeclaration {
                id: result_case,
                identity: "Success".into(),
                fields: Vec::new(),
            }],
        },
    }];
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: result_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: operation_place,
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(1),
                structural_type,
            },
        },
        StructuralPlaceDeclaration {
            id: result_place,
            kind: semantic_vocabulary::StructuralPlaceKind::Result,
        },
    ];
    machine.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation_id(1),
        result: OperationResult::Structural(StructuralOperationResult {
            place: operation_place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishScalarCase {
            result_case,
            fields: vec![],
        },
    }];
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(1),
        source: operation_place,
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    module
}

pub(super) fn payloadless_call_module() -> TerminalModule {
    let mut module = payloadless_case_module();
    let structural_type = structural_type_id(1);
    let mut callee = module.machines.remove(0);
    callee.id = machine_id(2);
    callee.entry = block_id(2);
    callee.blocks[0].id = block_id(2);
    let Terminator::ReturnStructural { edge, .. } = &mut callee.blocks[0].terminator else {
        unreachable!()
    };
    *edge = edge_id(2);
    callee.contract.id = contract_id(2);

    let caller = TerminalMachine {
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
            place: place_id(4),
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }),
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: place_id(3),
                kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    producer: operation_id(2),
                    structural_type,
                },
            },
            StructuralPlaceDeclaration {
                id: place_id(4),
                kind: semantic_vocabulary::StructuralPlaceKind::Result,
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
                id: operation_id(2),
                result: OperationResult::Structural(StructuralOperationResult {
                    place: place_id(3),
                    structural_type,
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
                source: place_id(3),
                returned_claims: Vec::new(),
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
    };
    module.machines = vec![caller, callee];
    module
}

pub(super) fn unit_module() -> TerminalModule {
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
                operations: Vec::new(),
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

pub(super) fn nearest_fma_module(operands: [IeeeFloatValue; 3]) -> TerminalModule {
    let format = operands[0].format();
    assert!(operands.iter().all(|operand| operand.format() == format));
    let mut module = unit_module();
    let operand_ids = [value_id(1), value_id(2), value_id(3)];
    let result = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(4),
        scalar_type: ScalarType::IeeeFloat(format),
    };
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(5),
        scalar_type: result.scalar_type,
    });
    machine.blocks[0].operations = operands
        .into_iter()
        .zip(operand_ids)
        .enumerate()
        .map(|(index, (value, id))| Operation {
            static_reach_binding: None,
            id: operation_id(index as u64 + 1),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id,
                scalar_type: ScalarType::IeeeFloat(format),
            }),
            kind: OperationKind::IeeeFloatConstant { value },
        })
        .chain(std::iter::once(Operation {
            static_reach_binding: None,
            id: operation_id(4),
            result: OperationResult::Scalar(result),
            kind: OperationKind::NearestIeeeFloatFusedMultiplyAdd {
                left: operand_ids[0],
                right: operand_ids[1],
                addend: operand_ids[2],
            },
        }))
        .collect();
    machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: result.id,
        cleanup_actions: Vec::new(),
    };
    module
}
