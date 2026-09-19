//! Nominal affine and partial affine field module fixtures.

use super::{
    block_id, contract_id, edge_id, empty_contract, machine_id, operation_id, place_id,
    structural_type_id,
};
use semantic_vocabulary::ScalarType;
use terminal_psi::{
    BindingRelevance, Block, NominalAffineCleanup, Operation, OperationKind, OperationResult,
    StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    VocabularyMarker,
};

pub(super) fn nominal_affine_module() -> TerminalModule {
    let token = StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![token.clone()],
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
                structural_places: vec![StructuralPlaceDeclaration {
                    id: place_id(1),
                    kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
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
                            place: place_id(1),
                            structural_type: token.id,
                            cleanup_machine: machine_id(2),
                            cleanup_receiver: None,
                            requirement_obligations: Vec::new(),
                        }],
                    },
                }],
                contract: empty_contract(contract_id(1)),
            },
            TerminalMachine {
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
            },
        ],
    }
}

pub(super) fn ordered_empty_nominal_affine_module(same_target: bool) -> TerminalModule {
    let mut module = nominal_affine_module();
    let first_type = structural_type_id(1);
    let second_type = if same_target {
        first_type
    } else {
        let second_type = structural_type_id(2);
        module.structural_types.push(StructuralTypeDeclaration {
            id: second_type,
            identity: "SecondToken".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    identity: "payload".into(),
                    id: semantic_vocabulary::StructuralFieldId::new(2).unwrap(),
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            64,
                        )
                        .unwrap(),
                    )),
                    relevance: BindingRelevance::Relevant,
                }],
            },
        });
        second_type
    };
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            identity: "payload".into(),
            id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    32,
                )
                .unwrap(),
            )),
            relevance: BindingRelevance::Relevant,
        }],
    };
    let second_cleanup_machine = if same_target {
        machine_id(2)
    } else {
        let mut target = module.machines[1].clone();
        target.id = machine_id(3);
        target.attachment = Some(second_type);
        target.entry = block_id(3);
        target.blocks[0].id = block_id(3);
        target.blocks[0].terminator = Terminator::ReturnUnit {
            edge: edge_id(3),
            trivial_affine_discards: Vec::new(),
        };
        target.contract.id = contract_id(3);
        module.machines.push(target);
        machine_id(3)
    };
    let caller = &mut module.machines[0];
    caller
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(2),
            position: 1,
            is_self: false,
            structural_type: second_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(2),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    caller.blocks[0].terminator = Terminator::ReturnUnitNominalAffine {
        edge: edge_id(1),
        cleanups: vec![
            NominalAffineCleanup {
                place: place_id(2),
                structural_type: second_type,
                cleanup_machine: second_cleanup_machine,
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
            NominalAffineCleanup {
                place: place_id(1),
                structural_type: first_type,
                cleanup_machine: machine_id(2),
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
        ],
    };
    module
}

pub(super) fn ordered_one_executable_nominal_affine_module() -> TerminalModule {
    let mut module = ordered_empty_nominal_affine_module(false);
    let helper_type = structural_type_id(3);
    module.structural_types.push(StructuralTypeDeclaration {
        id: helper_type,
        identity: "Helper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut helper = module.machines[1].clone();
    helper.id = machine_id(4);
    helper.attachment = Some(helper_type);
    helper.entry = block_id(4);
    helper.blocks[0].id = block_id(4);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(4),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(4);
    module.machines[2].blocks[0].operations.push(Operation {
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
    module.machines.push(helper);
    module
}

pub(super) fn three_ordered_empty_nominal_affine_module(same_target: bool) -> TerminalModule {
    let mut module = ordered_empty_nominal_affine_module(same_target);
    let third_type = if same_target {
        structural_type_id(1)
    } else {
        let third_type = structural_type_id(3);
        module.structural_types.push(StructuralTypeDeclaration {
            id: third_type,
            identity: "ThirdToken".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    identity: "payload".into(),
                    id: semantic_vocabulary::StructuralFieldId::new(3).unwrap(),
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            64,
                        )
                        .unwrap(),
                    )),
                    relevance: BindingRelevance::Relevant,
                }],
            },
        });
        third_type
    };
    let third_cleanup_machine = if same_target {
        machine_id(2)
    } else {
        let mut target = module.machines[1].clone();
        target.id = machine_id(4);
        target.attachment = Some(third_type);
        target.entry = block_id(4);
        target.blocks[0].id = block_id(4);
        target.blocks[0].terminator = Terminator::ReturnUnit {
            edge: edge_id(4),
            trivial_affine_discards: Vec::new(),
        };
        target.contract.id = contract_id(4);
        module.machines.push(target);
        machine_id(4)
    };
    let caller = &mut module.machines[0];
    caller
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(3),
            position: 2,
            is_self: false,
            structural_type: third_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(3),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 2,
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
            place: place_id(3),
            structural_type: third_type,
            cleanup_machine: third_cleanup_machine,
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        },
    );
    module
}

pub(super) fn ordered_two_distinct_executable_nominal_affine_module() -> TerminalModule {
    let mut module = ordered_one_executable_nominal_affine_module();
    let helper_type = structural_type_id(4);
    module.structural_types.push(StructuralTypeDeclaration {
        id: helper_type,
        identity: "SecondHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut helper = module.machines[3].clone();
    helper.id = machine_id(5);
    helper.attachment = Some(helper_type);
    helper.entry = block_id(5);
    helper.blocks[0].id = block_id(5);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(5),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(5);
    module.machines[1].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(2),
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

pub(super) fn ordered_shared_executable_nominal_affine_module() -> TerminalModule {
    let mut module = ordered_empty_nominal_affine_module(true);
    let helper_type = structural_type_id(2);
    module.structural_types.push(StructuralTypeDeclaration {
        id: helper_type,
        identity: "SharedHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let mut helper = module.machines[1].clone();
    helper.id = machine_id(3);
    helper.attachment = Some(helper_type);
    helper.entry = block_id(3);
    helper.blocks[0].id = block_id(3);
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(3),
        trivial_affine_discards: Vec::new(),
    };
    helper.contract.id = contract_id(3);
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
    module.machines.push(helper);
    module
}

pub(super) fn three_ordered_shared_executable_nominal_affine_module() -> TerminalModule {
    let mut module = ordered_shared_executable_nominal_affine_module();
    let caller = &mut module.machines[0];
    caller
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(3),
            position: 2,
            is_self: false,
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(3),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 2,
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
            place: place_id(3),
            structural_type: structural_type_id(1),
            cleanup_machine: machine_id(2),
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        },
    );
    module
}

pub(super) fn executable_nominal_affine_module() -> TerminalModule {
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

pub(super) fn two_helper_nominal_affine_module() -> TerminalModule {
    let mut module = executable_nominal_affine_module();
    let second_helper_type = StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "SecondHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(second_helper_type.clone());
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
    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(4),
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
        entry: block_id(4),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(4),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(4),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(4)),
    });
    module
}

pub(super) fn three_helper_nominal_affine_module() -> TerminalModule {
    let mut module = two_helper_nominal_affine_module();
    let third_helper_type = StructuralTypeDeclaration {
        id: structural_type_id(4),
        identity: "ThirdHelper".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    module.structural_types.push(third_helper_type.clone());
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
    third_helper.attachment = Some(third_helper_type.id);
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

pub(super) fn partial_affine_field_module() -> TerminalModule {
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
                    id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                    identity: "left".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(token.id),
                },
                StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(2).unwrap(),
                    identity: "right".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(token.id),
                },
            ],
        },
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
            structural_type: pair.id,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: place_id(1),
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
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
                residual_affine_discards: vec![StructuralAffineDiscard {
                    place: place_id(1),
                    path: vec![StructuralPathSegment::Field("left".into())],
                    structural_type: token.id,
                }],
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
        structural_parameters: vec![StructuralParameterDeclaration {
            place: place_id(2),
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
        structural_places: vec![StructuralPlaceDeclaration {
            id: place_id(2),
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
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
        structural_types: vec![token, pair],
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
