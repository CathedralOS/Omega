//! Fixtures shared by the fixed entry fuel tests: nominal affine cleanup
//! fixtures and bound assertions.

#[path = "fixed_entry/call_and_outcome_bounds.rs"]
mod call_and_outcome_bounds;
#[path = "fixed_entry/entry_bounds.rs"]
mod entry_bounds;

use proof_admission::{EvidenceRoute, PrimitiveJudgment};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, ContractId, EdgeId, IntegerSign, IntegerType,
    IntegerValue, MachineId, ObligationId, OperationId, PlaceId, Proposition, ScalarTerm,
    ScalarType, ServiceId, StructuralCaseId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, ContractClause, MachineContract, NominalAffineCleanup,
    Operation, OperationKind, OperationResult, ServiceDeclaration, StructuralAccess,
    StructuralArgument, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{ObligationEvidence, ProofBundle};

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

fn ordered_empty_nominal_affine_fixture(same_target: bool) -> TerminalModule {
    let first_type = structural_type_id(900);
    let second_type = if same_target {
        first_type
    } else {
        structural_type_id(901)
    };
    let first_place = place_id(900);
    let second_place = place_id(901);
    let first_cleanup = machine_id(901);
    let second_cleanup = if same_target {
        first_cleanup
    } else {
        machine_id(902)
    };
    let primitive_record = |id, identity: &str, field| StructuralTypeDeclaration {
        id,
        identity: identity.into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                identity: "payload".into(),
                id: semantic_vocabulary::StructuralFieldId::new(field).unwrap(),
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                )),
                relevance: terminal_psi::BindingRelevance::Relevant,
            }],
        },
    };
    let cleanup_machine = |id, attachment, block, edge, contract| TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id,
        attachment: Some(attachment),
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
        entry: block,
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge,
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: contract,
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };

    let mut module = unit_fixture();
    module.structural_types = vec![primitive_record(first_type, "test::First", 900)];
    if !same_target {
        module
            .structural_types
            .push(primitive_record(second_type, "test::Second", 901));
    }
    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![
        StructuralParameterDeclaration {
            access: StructuralAccess::Owned,
            place: first_place,
            position: 0,
            is_self: false,
            structural_type: first_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
        StructuralParameterDeclaration {
            access: StructuralAccess::Owned,
            place: second_place,
            position: 1,
            is_self: false,
            structural_type: second_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
    ];
    caller.structural_places = vec![
        StructuralPlaceDeclaration {
            id: first_place,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: second_place,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
    ];
    caller.blocks[0].terminator = Terminator::ReturnUnitNominalAffine {
        edge: edge_id(900),
        cleanups: vec![
            NominalAffineCleanup {
                place: second_place,
                structural_type: second_type,
                cleanup_machine: second_cleanup,
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
            NominalAffineCleanup {
                place: first_place,
                structural_type: first_type,
                cleanup_machine: first_cleanup,
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
        ],
    };
    module.machines.push(cleanup_machine(
        first_cleanup,
        first_type,
        block_id(901),
        edge_id(901),
        contract_id(901),
    ));
    if !same_target {
        module.machines.push(cleanup_machine(
            second_cleanup,
            second_type,
            block_id(902),
            edge_id(902),
            contract_id(902),
        ));
    }
    module
}

fn ordered_one_executable_nominal_affine_fixture() -> TerminalModule {
    let mut module = ordered_empty_nominal_affine_fixture(false);
    let helper_type = structural_type_id(902);
    module.structural_types.push(StructuralTypeDeclaration {
        id: helper_type,
        identity: "test::Helper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut helper = module.machines[1].clone();
    helper.id = machine_id(903);
    helper.attachment = Some(helper_type);
    helper.entry = block_id(903);
    helper.blocks[0].id = block_id(903);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(903),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(903);
    module.machines[2].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(903),
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
    module
}

fn ordered_two_distinct_executable_nominal_affine_fixture() -> TerminalModule {
    let mut module = ordered_one_executable_nominal_affine_fixture();
    let helper_type = structural_type_id(903);
    module.structural_types.push(StructuralTypeDeclaration {
        id: helper_type,
        identity: "test::SecondHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut helper = module.machines[3].clone();
    helper.id = machine_id(904);
    helper.attachment = Some(helper_type);
    helper.entry = block_id(904);
    helper.blocks[0].id = block_id(904);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(904),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(904);
    module.machines[1].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(904),
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
    module
}

fn ordered_shared_executable_nominal_affine_fixture() -> TerminalModule {
    let mut module = ordered_empty_nominal_affine_fixture(true);
    let helper_type = structural_type_id(901);
    module.structural_types.push(StructuralTypeDeclaration {
        id: helper_type,
        identity: "test::SharedHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut helper = module.machines[1].clone();
    helper.id = machine_id(902);
    helper.attachment = Some(helper_type);
    helper.entry = block_id(902);
    helper.blocks[0].id = block_id(902);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(902),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(902);
    module.machines[1].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(902),
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
    module
}

fn three_ordered_shared_executable_nominal_affine_fixture() -> TerminalModule {
    let mut module = ordered_shared_executable_nominal_affine_fixture();
    let caller = &mut module.machines[0];
    caller
        .structural_parameters
        .push(StructuralParameterDeclaration {
            access: StructuralAccess::Owned,
            place: place_id(902),
            position: 2,
            is_self: false,
            structural_type: structural_type_id(900),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(902),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 2,
            is_self: false,
        },
    });
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &mut caller.blocks[0].terminator
    else {
        unreachable!("ordered fixture retains nominal cleanup")
    };
    cleanups.insert(
        0,
        NominalAffineCleanup {
            place: place_id(902),
            structural_type: structural_type_id(900),
            cleanup_machine: machine_id(901),
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        },
    );
    module
}

fn five_ordered_shared_executable_nominal_affine_fixture() -> TerminalModule {
    let mut module = three_ordered_shared_executable_nominal_affine_fixture();
    let caller = &mut module.machines[0];
    for position in 3_u32..5 {
        let place = place_id(u64::from(position) + 900);
        caller
            .structural_parameters
            .push(StructuralParameterDeclaration {
                access: StructuralAccess::Owned,
                place,
                position,
                is_self: false,
                structural_type: structural_type_id(900),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
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
                structural_type: structural_type_id(900),
                cleanup_machine: machine_id(901),
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
        );
    }
    module
}

fn executable_nominal_affine_fixture() -> TerminalModule {
    let empty_contract = |raw| MachineContract {
        erased_scalar_formals: Vec::new(),
        id: contract_id(raw),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    };
    let token_type = structural_type_id(900);
    let helper_type = structural_type_id(901);
    let source = place_id(900);
    let mut module = unit_fixture();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: token_type,
            identity: "test::Token".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    identity: "payload".into(),
                    id: semantic_vocabulary::StructuralFieldId::new(900).unwrap(),
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    )),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                }],
            },
        },
        StructuralTypeDeclaration {
            id: helper_type,
            identity: "test::Helper".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
    ];
    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![StructuralParameterDeclaration {
        access: StructuralAccess::Owned,
        place: source,
        position: 0,
        is_self: false,
        structural_type: token_type,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    caller.structural_places = vec![StructuralPlaceDeclaration {
        id: source,
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    caller.blocks[0].terminator = Terminator::ReturnUnitNominalAffine {
        edge: edge_id(900),
        cleanups: vec![NominalAffineCleanup {
            place: source,
            structural_type: token_type,
            cleanup_machine: machine_id(901),
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        }],
    };
    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(901),
        attachment: Some(token_type),
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
        entry: block_id(901),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(901),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: operation_id(901),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    erased_arguments: Vec::new(),
                    arguments: Vec::new(),
                    callee: machine_id(902),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: edge_id(901),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(901),
    });
    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(902),
        attachment: Some(helper_type),
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
        entry: block_id(902),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(902),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(902),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(902),
    });
    module
}

fn two_helper_nominal_affine_fixture() -> TerminalModule {
    let mut module = executable_nominal_affine_fixture();
    let second_helper_type = StructuralTypeDeclaration {
        id: structural_type_id(902),
        identity: "test::SecondHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(second_helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(902),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            arguments: Vec::new(),
            callee: machine_id(903),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(903),
        attachment: Some(second_helper_type.id),
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
        entry: block_id(903),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(903),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(903),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: contract_id(903),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    });
    module
}

fn three_helper_nominal_affine_fixture() -> TerminalModule {
    let mut module = two_helper_nominal_affine_fixture();
    let third_helper_type = StructuralTypeDeclaration {
        id: structural_type_id(903),
        identity: "test::ThirdHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(third_helper_type.clone());
    module.machines[1].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(903),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            arguments: Vec::new(),
            callee: machine_id(904),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    let mut third_helper = module.machines[2].clone();
    third_helper.id = machine_id(904);
    third_helper.attachment = Some(third_helper_type.id);
    third_helper.entry = block_id(904);
    third_helper.blocks[0].id = block_id(904);
    third_helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(904),
        trivial_affine_discards: Vec::new(),
    };
    third_helper.contract.id = contract_id(904);
    module.machines.push(third_helper);
    module
}

fn unit_effect_fixture() -> TerminalModule {
    let service = service_id(1);
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(700),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
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
            identity: "test::boundary".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
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
                id: machine_id(700),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: vec![service],
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(700),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(700),
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            id: operation_id(700),
                            result: OperationResult::Unit,
                            kind: OperationKind::CallUnit {
                                erased_arguments: Vec::new(),
                                arguments: Vec::new(),
                                callee: machine_id(701),
                                structural_arguments: Vec::new(),
                                claim_transfers: Vec::new(),
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            id: operation_id(701),
                            result: OperationResult::Unit,
                            kind: OperationKind::PortWrite {
                                service,
                                port: 0x20,
                                value: 0x20,
                            },
                        },
                    ],
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(700),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: contract_id(700),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(701),
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
                entry: block_id(701),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(701),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: operation_id(702),
                        result: OperationResult::Unit,
                        kind: OperationKind::BoundaryCall {
                            boundary: boundary_id(1),
                            arguments: Vec::new(),
                            structural_arguments: Vec::new(),
                            completion_receipts: Vec::new(),
                        },
                    }],
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(701),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: contract_id(701),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
        ],
    }
}

fn write_only_primitive_store_fixture() -> TerminalModule {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let structural_type = structural_type_id(960);
    let caller_place = place_id(960);
    let callee_place = place_id(961);
    let parameter = |place| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::WriteOnlyBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let structural_place = |id| StructuralPlaceDeclaration {
        id,
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    };
    let mut module = unit_effect_fixture();
    module.structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::WriteOnlyU8".into(),
        shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
    }];
    module.services.clear();
    module.root_service_reach = Default::default();
    module.boundary_machines.clear();

    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![parameter(caller_place)];
    caller.structural_places = vec![structural_place(caller_place)];
    caller.published_service_ceiling.clear();
    caller.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation_id(700),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            arguments: Vec::new(),
            callee: machine_id(701),
            structural_arguments: vec![StructuralArgument {
                place: caller_place,
                path: Vec::new(),
                access: StructuralAccess::WriteOnlyBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }];

    let callee = &mut module.machines[1];
    callee.structural_parameters = vec![parameter(callee_place)];
    callee.structural_places = vec![structural_place(callee_place)];
    callee.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: operation_id(702),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(960),
                scalar_type,
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(7),
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(703),
            result: OperationResult::Unit,
            kind: OperationKind::WriteOnlyPrimitiveStore {
                destination: callee_place,
                path: Vec::new(),
                value: value_id(960),
            },
        },
    ];
    module
}

fn fixture() -> (TerminalModule, ProofBundle) {
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let literal = ScalarTerm::integer(integer, IntegerValue::Signed(7)).unwrap();
    let goal = Proposition::Equal(literal.clone(), literal);
    let obligation = obligation_id(1);
    let module = TerminalModule {
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
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(3),
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
                        result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: value_id(1),
                            scalar_type,
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(7),
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
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(2),
                        value: value_id(2),
                    },
                },
            ],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: vec![goal.clone()],
                ensures: vec![ContractClause {
                    obligation,
                    proposition: goal,
                }],
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
        }],
    };
    (module, proof)
}

fn call_fixture() -> TerminalModule {
    let boolean = ScalarType::Boolean;
    let declaration = |raw| ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(raw),
        scalar_type: boolean,
    };
    let empty_contract = |raw| MachineContract {
        erased_scalar_formals: Vec::new(),
        id: contract_id(raw),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    };
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
                result: TerminalMachineResult::Scalar(declaration(3)),
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
                            result: terminal_psi::OperationResult::Scalar(declaration(1)),
                            kind: OperationKind::BooleanConstant { value: true },
                        },
                        Operation {
                            static_reach_binding: None,
                            id: operation_id(2),
                            result: terminal_psi::OperationResult::Scalar(declaration(2)),
                            kind: OperationKind::Call {
                                erased_arguments: Vec::new(),
                                callee: machine_id(2),
                                arguments: vec![value_id(1)],
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                        },
                    ],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(1),
                        value: value_id(2),
                    },
                }],
                contract: empty_contract(1),
            },
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(2),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![declaration(4)],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(declaration(5)),
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
                        value: value_id(4),
                    },
                }],
                contract: empty_contract(2),
            },
        ],
    }
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
id_constructor!(operation_id, OperationId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);
id_constructor!(boundary_id, BoundaryMachineId);
id_constructor!(service_id, ServiceId);
id_constructor!(place_id, PlaceId);
id_constructor!(claim_id, ClaimId);
id_constructor!(structural_type_id, StructuralTypeId);
id_constructor!(structural_case_id, StructuralCaseId);
