use psi_core::{
    BlockId, ContractId, EdgeId, MachineId, OperationId, PlaceId, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use psi_terminal::{
    Block, MachineContract, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalBorrowBoundarySource, TerminalBorrowPlace, TerminalMachine,
    TerminalMachineResult, TerminalModule, TerminalReborrowRestorationClass,
    TerminalReborrowRestoredCallUse, TerminalReborrowSharedCohortMember, Terminator,
    VocabularyMarker,
};
use psi_terminal_verifier::{ModuleError, validate_module};

fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero fixture identity")
}

fn borrow_identity(digit: char) -> String {
    format!("terminal-borrow:{}", digit.to_string().repeat(64))
}

fn statement(statement_index: u64) -> TerminalBorrowBoundarySource {
    TerminalBorrowBoundarySource::Statement { statement_index }
}

fn restored_call_use_module() -> TerminalModule {
    let caller = id(1, MachineId::new);
    let callee = id(2, MachineId::new);
    let structural_type = id(1, StructuralTypeId::new);
    let caller_place = id(1, PlaceId::new);
    let callee_place = id(2, PlaceId::new);
    let operation = id(1, OperationId::new);
    let parameter = |place, position| StructuralParameterDeclaration {
        place,
        position,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let contract = |raw| MachineContract {
        id: id(raw, ContractId::new),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    };
    let root_identity = borrow_identity('d');
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "RestoredCell".to_owned(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: vec![TerminalReborrowRestoredCallUse {
            machine: caller,
            operation,
            restoration_class: TerminalReborrowRestorationClass::ExclusiveReactivation,
            call_boundary: TerminalBorrowBoundarySource::Call {
                statement_index: 2,
                call_ordinal: 0,
                target_identity: borrow_identity('f'),
            },
            call_target_machine: callee,
            source_machine_identity: borrow_identity('a'),
            source_state_identity: borrow_identity('b'),
            direct_root_owner_identity: borrow_identity('c'),
            direct_root_owner_path: Vec::new(),
            direct_root_place: TerminalBorrowPlace {
                root_identity: root_identity.clone(),
                segments: Vec::new(),
            },
            direct_root_activation: statement(0),
            direct_root_weakening: statement(4),
            direct_root_lifetime_identity: root_identity.clone(),
            child_owner_identity: borrow_identity('e'),
            child_owner_path: Vec::new(),
            child_place: TerminalBorrowPlace {
                root_identity,
                segments: Vec::new(),
            },
            projection_remainder: Vec::new(),
            child_access: StructuralAccess::WriteOnlyBorrow,
            child_activation: statement(1),
            formation_boundary: statement(1),
            child_weakening: statement(2),
            shared_cohort: Vec::new(),
        }],
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
                id: caller,
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: vec![parameter(caller_place, 0)],
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: vec![StructuralPlaceDeclaration {
                    id: caller_place,
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
                entry: id(1, BlockId::new),
                blocks: vec![Block {
                    id: id(1, BlockId::new),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: operation,
                        result: OperationResult::Unit,
                        kind: OperationKind::CallUnit {
                            arguments: Vec::new(),
                            callee,
                            structural_arguments: vec![StructuralArgument {
                                place: caller_place,
                                path: Vec::new(),
                                access: StructuralAccess::MutableBorrow,
                            }],
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                    }],
                    terminator: Terminator::ReturnUnit {
                        edge: id(1, EdgeId::new),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: contract(1),
            },
            TerminalMachine {
                id: callee,
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: vec![parameter(callee_place, 0)],
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: vec![StructuralPlaceDeclaration {
                    id: callee_place,
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
                entry: id(2, BlockId::new),
                blocks: vec![Block {
                    id: id(2, BlockId::new),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: id(2, EdgeId::new),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: contract(2),
            },
        ],
    }
}

fn assert_invalid(mutator: impl FnOnce(&mut TerminalModule)) {
    let mut module = restored_call_use_module();
    mutator(&mut module);
    let result = validate_module(&module);
    assert!(
        matches!(
            &result,
            Err(ModuleError::InvalidReborrowRestoredCallUse { .. })
        ),
        "unexpected validation result: {result:?}"
    );
}

#[test]
fn exact_restored_parent_call_use_validates() {
    for child_access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
        StructuralAccess::SharedBorrow,
    ] {
        let mut module = restored_call_use_module();
        module.reborrow_restored_call_uses[0].child_access = child_access;
        module.reborrow_restored_call_uses[0].restoration_class =
            if child_access == StructuralAccess::SharedBorrow {
                TerminalReborrowRestorationClass::SharedFreezeRestoration
            } else {
                TerminalReborrowRestorationClass::ExclusiveReactivation
            };
        if child_access == StructuralAccess::SharedBorrow {
            let row = &mut module.reborrow_restored_call_uses[0];
            row.shared_cohort = vec![TerminalReborrowSharedCohortMember {
                child_owner_identity: row.child_owner_identity.clone(),
                child_owner_path: row.child_owner_path.clone(),
                child_place: row.child_place.clone(),
                child_access: row.child_access,
                child_activation: row.child_activation.clone(),
                child_weakening: row.child_weakening.clone(),
            }];
        }
        validate_module(&module).expect("exact restored-parent call use");
    }
}

#[test]
fn exact_two_and_three_member_shared_freeze_cohorts_validate_and_fence_roster_drift() {
    let mut module = restored_call_use_module();
    let observer = id(3, MachineId::new);
    let observer_left_place = id(3, PlaceId::new);
    let observer_right_place = id(4, PlaceId::new);
    let structural_type = module.structural_types[0].id;
    let caller_place = module.machines[0].structural_parameters[0].place;
    module.machines[0].blocks[0].operations.insert(
        0,
        Operation {
            id: id(2, OperationId::new),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                arguments: Vec::new(),
                callee: observer,
                structural_arguments: vec![
                    StructuralArgument {
                        place: caller_place,
                        path: Vec::new(),
                        access: StructuralAccess::SharedBorrow,
                    },
                    StructuralArgument {
                        place: caller_place,
                        path: Vec::new(),
                        access: StructuralAccess::SharedBorrow,
                    },
                ],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        },
    );
    let shared_parameter = |place, position| StructuralParameterDeclaration {
        place,
        position,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    module.machines.push(TerminalMachine {
        id: observer,
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![
            shared_parameter(observer_left_place, 0),
            shared_parameter(observer_right_place, 1),
        ],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: observer_left_place,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: observer_right_place,
                kind: StructuralPlaceKind::Parameter {
                    position: 1,
                    is_self: false,
                },
            },
        ],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: id(3, BlockId::new),
        blocks: vec![Block {
            id: id(3, BlockId::new),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: id(3, EdgeId::new),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: id(3, ContractId::new),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    });
    {
        let row = &mut module.reborrow_restored_call_uses[0];
        row.restoration_class = TerminalReborrowRestorationClass::SharedFreezeRestoration;
        row.child_access = StructuralAccess::SharedBorrow;
        row.call_boundary = TerminalBorrowBoundarySource::Call {
            statement_index: 4,
            call_ordinal: 0,
            target_identity: borrow_identity('f'),
        };
        row.child_weakening = statement(4);
        row.direct_root_weakening = statement(6);
        let primary = TerminalReborrowSharedCohortMember {
            child_owner_identity: row.child_owner_identity.clone(),
            child_owner_path: row.child_owner_path.clone(),
            child_place: row.child_place.clone(),
            child_access: row.child_access,
            child_activation: row.child_activation.clone(),
            child_weakening: row.child_weakening.clone(),
        };
        let mut sibling = primary.clone();
        sibling.child_owner_identity = borrow_identity('9');
        sibling.child_activation = statement(2);
        row.shared_cohort = vec![primary, sibling];
    }
    validate_module(&module).expect("exact two-member shared-freeze cohort");

    let mut three = module.clone();
    let third_place = id(5, PlaceId::new);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut three.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments.push(StructuralArgument {
        place: caller_place,
        path: Vec::new(),
        access: StructuralAccess::SharedBorrow,
    });
    three.machines[2]
        .structural_parameters
        .push(shared_parameter(third_place, 2));
    three.machines[2]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: third_place,
            kind: StructuralPlaceKind::Parameter {
                position: 2,
                is_self: false,
            },
        });
    let mut member = three.reborrow_restored_call_uses[0].shared_cohort[1].clone();
    member.child_owner_identity = borrow_identity('8');
    member.child_activation = statement(3);
    three.reborrow_restored_call_uses[0]
        .shared_cohort
        .push(member);
    validate_module(&three).expect("exact three-member shared-freeze cohort");

    let mut fourth = three.clone();
    let mut member = fourth.reborrow_restored_call_uses[0].shared_cohort[2].clone();
    member.child_owner_identity = borrow_identity('7');
    member.child_activation = statement(4);
    fourth.reborrow_restored_call_uses[0]
        .shared_cohort
        .push(member);
    assert!(matches!(
        validate_module(&fourth),
        Err(ModuleError::InvalidReborrowRestoredCallUse { .. })
    ));

    let mut nonadjacent_duplicate = three.clone();
    nonadjacent_duplicate.reborrow_restored_call_uses[0].shared_cohort[2] =
        nonadjacent_duplicate.reborrow_restored_call_uses[0].shared_cohort[0].clone();
    assert!(matches!(
        validate_module(&nonadjacent_duplicate),
        Err(ModuleError::InvalidReborrowRestoredCallUse { .. })
    ));

    let mut duplicate = module.clone();
    duplicate.reborrow_restored_call_uses[0].shared_cohort[1] =
        duplicate.reborrow_restored_call_uses[0].shared_cohort[0].clone();
    assert!(matches!(
        validate_module(&duplicate),
        Err(ModuleError::InvalidReborrowRestoredCallUse { .. })
    ));

    let mut missing_observation = module.clone();
    missing_observation.machines[0].blocks[0]
        .operations
        .remove(0);
    assert!(matches!(
        validate_module(&missing_observation),
        Err(ModuleError::InvalidReborrowRestoredCallUse { .. })
    ));

    let mut reordered = module.clone();
    reordered.reborrow_restored_call_uses[0]
        .shared_cohort
        .swap(0, 1);
    assert!(matches!(
        validate_module(&reordered),
        Err(ModuleError::InvalidReborrowRestoredCallUse { .. })
    ));

    let mut reordered_three = three;
    reordered_three.reborrow_restored_call_uses[0]
        .shared_cohort
        .swap(1, 2);
    assert!(matches!(
        validate_module(&reordered_three),
        Err(ModuleError::InvalidReborrowRestoredCallUse { .. })
    ));

    let mut same_activation = module.clone();
    same_activation.reborrow_restored_call_uses[0].shared_cohort[1].child_activation = statement(1);
    assert!(matches!(
        validate_module(&same_activation),
        Err(ModuleError::InvalidReborrowRestoredCallUse { .. })
    ));

    let mut mismatched = module;
    mismatched.reborrow_restored_call_uses[0].shared_cohort[1].child_weakening = statement(3);
    assert!(matches!(
        validate_module(&mismatched),
        Err(ModuleError::InvalidReborrowRestoredCallUse { .. })
    ));
}

#[test]
fn restored_parent_call_use_rejects_call_shape_substitution() {
    assert_invalid(|module| {
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(0)];
    });
    assert_invalid(|module| {
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        structural_arguments[0].access = StructuralAccess::SharedBorrow;
    });
    assert_invalid(|module| {
        module.machines[1].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    });
    assert_invalid(|module| {
        module.machines[1].attachment = Some(id(1, StructuralTypeId::new));
        module.machines[1].structural_parameters[0].is_self = true;
        module.machines[1].structural_places[0].kind = StructuralPlaceKind::Parameter {
            position: 0,
            is_self: true,
        };
    });
    assert_invalid(|module| {
        module.machines[1].result = TerminalMachineResult::Scalar(psi_terminal::ValueDeclaration {
            id: id(1, ValueId::new),
            scalar_type: psi_core::ScalarType::Boolean,
        });
    });
}

#[test]
fn restored_parent_call_use_rejects_identity_and_lineage_substitution() {
    assert_invalid(|module| {
        module.reborrow_restored_call_uses[0].source_state_identity =
            format!("terminal-borrow:{}", "A".repeat(64));
    });
    assert_invalid(|module| {
        module.reborrow_restored_call_uses[0].direct_root_lifetime_identity = borrow_identity('f');
    });
    assert_invalid(|module| {
        module.reborrow_restored_call_uses[0].direct_root_weakening = statement(2);
    });
    assert_invalid(|module| {
        module.reborrow_restored_call_uses[0].direct_root_activation = statement(2);
    });
    assert_invalid(|module| {
        module.reborrow_restored_call_uses[0].child_access = StructuralAccess::Owned;
    });
    assert_invalid(|module| {
        module.reborrow_restored_call_uses[0].restoration_class =
            TerminalReborrowRestorationClass::SharedFreezeRestoration;
    });

    let mut shared = restored_call_use_module();
    {
        let row = &mut shared.reborrow_restored_call_uses[0];
        row.restoration_class = TerminalReborrowRestorationClass::SharedFreezeRestoration;
        row.child_access = StructuralAccess::SharedBorrow;
        row.shared_cohort = vec![TerminalReborrowSharedCohortMember {
            child_owner_identity: row.child_owner_identity.clone(),
            child_owner_path: row.child_owner_path.clone(),
            child_place: row.child_place.clone(),
            child_access: row.child_access,
            child_activation: row.child_activation.clone(),
            child_weakening: row.child_weakening.clone(),
        }];
    }
    let mut missing = shared.clone();
    missing.reborrow_restored_call_uses[0].shared_cohort.clear();
    assert!(matches!(
        validate_module(&missing),
        Err(ModuleError::InvalidReborrowRestoredCallUse { .. })
    ));
    let mut duplicate = shared.clone();
    let member = duplicate.reborrow_restored_call_uses[0].shared_cohort[0].clone();
    duplicate.reborrow_restored_call_uses[0]
        .shared_cohort
        .push(member);
    assert!(matches!(
        validate_module(&duplicate),
        Err(ModuleError::InvalidReborrowRestoredCallUse { .. })
    ));
    let mut lookalike_operation = shared.clone();
    let mut second_operation = lookalike_operation.machines[0].blocks[0].operations[0].clone();
    second_operation.id = id(2, OperationId::new);
    lookalike_operation.machines[0].blocks[0]
        .operations
        .push(second_operation);
    assert!(matches!(
        validate_module(&lookalike_operation),
        Err(ModuleError::InvalidReborrowRestoredCallUse { .. })
    ));
    shared.reborrow_restored_call_uses[0].shared_cohort[0].child_owner_identity =
        borrow_identity('9');
    assert!(matches!(
        validate_module(&shared),
        Err(ModuleError::InvalidReborrowRestoredCallUse { .. })
    ));
    assert_invalid(|module| {
        module.reborrow_restored_call_uses[0].call_boundary = TerminalBorrowBoundarySource::Call {
            statement_index: 1,
            call_ordinal: 0,
            target_identity: borrow_identity('f'),
        };
    });
    assert_invalid(|module| {
        let TerminalBorrowBoundarySource::Call { call_ordinal, .. } =
            &mut module.reborrow_restored_call_uses[0].call_boundary
        else {
            unreachable!()
        };
        *call_ordinal = 1;
    });
    assert_invalid(|module| {
        module.reborrow_restored_call_uses[0].call_target_machine = id(3, MachineId::new);
    });
    assert_invalid(|module| {
        module.reborrow_restored_call_uses[0]
            .child_place
            .segments
            .push(psi_terminal::TerminalBorrowPlaceSegment::FixedIndex(0));
    });
}

#[test]
fn restored_parent_call_uses_are_unique_and_canonically_ordered() {
    let mut duplicate = restored_call_use_module();
    duplicate
        .reborrow_restored_call_uses
        .push(duplicate.reborrow_restored_call_uses[0].clone());
    assert!(matches!(
        validate_module(&duplicate),
        Err(ModuleError::DuplicateReborrowRestoredCallUse)
    ));

    let mut duplicate_lifecycle = restored_call_use_module();
    let mut second_operation = duplicate_lifecycle.machines[0].blocks[0].operations[0].clone();
    second_operation.id = id(2, OperationId::new);
    duplicate_lifecycle.machines[0].blocks[0]
        .operations
        .push(second_operation);
    let mut second_row = duplicate_lifecycle.reborrow_restored_call_uses[0].clone();
    second_row.operation = id(2, OperationId::new);
    second_row.source_machine_identity = borrow_identity('f');
    second_row.source_state_identity = borrow_identity('9');
    second_row.child_owner_identity = borrow_identity('8');
    duplicate_lifecycle
        .reborrow_restored_call_uses
        .push(second_row);
    assert!(matches!(
        validate_module(&duplicate_lifecycle),
        Err(ModuleError::DuplicateReborrowRestoredCallLifecycle)
    ));

    let mut duplicate_call_coordinate = restored_call_use_module();
    let mut second_operation =
        duplicate_call_coordinate.machines[0].blocks[0].operations[0].clone();
    second_operation.id = id(2, OperationId::new);
    duplicate_call_coordinate.machines[0].blocks[0]
        .operations
        .push(second_operation);
    let mut second_row = duplicate_call_coordinate.reborrow_restored_call_uses[0].clone();
    second_row.operation = id(2, OperationId::new);
    second_row.child_activation = statement(2);
    second_row.formation_boundary = statement(2);
    second_row.call_boundary = TerminalBorrowBoundarySource::Call {
        statement_index: 2,
        call_ordinal: 0,
        target_identity: borrow_identity('9'),
    };
    duplicate_call_coordinate
        .reborrow_restored_call_uses
        .push(second_row);
    assert!(matches!(
        validate_module(&duplicate_call_coordinate),
        Err(ModuleError::DuplicateReborrowRestoredCallLifecycle)
    ));

    let mut reordered = restored_call_use_module();
    let mut second_operation = reordered.machines[0].blocks[0].operations[0].clone();
    second_operation.id = id(2, OperationId::new);
    reordered.machines[0].blocks[0]
        .operations
        .push(second_operation);
    let mut second_row = reordered.reborrow_restored_call_uses[0].clone();
    second_row.operation = id(2, OperationId::new);
    second_row.child_owner_identity = borrow_identity('f');
    second_row.child_weakening = statement(3);
    second_row.call_boundary = TerminalBorrowBoundarySource::Call {
        statement_index: 3,
        call_ordinal: 0,
        target_identity: borrow_identity('f'),
    };
    reordered.reborrow_restored_call_uses.push(second_row);
    reordered.reborrow_restored_call_uses.reverse();
    assert!(matches!(
        validate_module(&reordered),
        Err(ModuleError::NonCanonicalReborrowRestoredCallUseOrder)
    ));
}
