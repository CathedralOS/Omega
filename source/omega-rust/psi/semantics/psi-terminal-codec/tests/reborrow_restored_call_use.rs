use psi_core::{
    BlockId, ContractId, EdgeId, MachineId, OperationId, PlaceId, StructuralPlaceKind,
    StructuralTypeId,
};
use psi_terminal::{
    Block, MachineContract, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalBorrowBoundarySource, TerminalBorrowPlace, TerminalMachine, TerminalMachineResult,
    TerminalModule, TerminalReborrowRestorationClass, TerminalReborrowRestoredCallUse,
    TerminalReborrowSharedCohortMember, Terminator, VocabularyMarker,
};
use psi_terminal_codec::{CodecError, decode_module, encode_module, semantic_fingerprint};
use psi_terminal_verifier::ModuleError;

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

#[test]
fn restored_call_use_round_trips_and_commits_every_variable_axis() {
    let module = restored_call_use_module();
    let encoded = encode_module(&module).expect("restored use should encode");
    assert_eq!(decode_module(&encoded), Ok(module.clone()));

    let original = semantic_fingerprint(&module).expect("restored use should fingerprint");
    let mut different_state = module.clone();
    different_state.reborrow_restored_call_uses[0].source_state_identity = borrow_identity('f');
    assert_ne!(
        semantic_fingerprint(&different_state).expect("different state remains valid"),
        original
    );
    let mut different_child_access = module;
    different_child_access.reborrow_restored_call_uses[0].child_access =
        StructuralAccess::MutableBorrow;
    assert_ne!(
        semantic_fingerprint(&different_child_access)
            .expect("different child access remains valid"),
        original
    );

    let mut shared = restored_call_use_module();
    shared.reborrow_restored_call_uses[0].child_access = StructuralAccess::SharedBorrow;
    shared.reborrow_restored_call_uses[0].restoration_class =
        TerminalReborrowRestorationClass::SharedFreezeRestoration;
    {
        let row = &mut shared.reborrow_restored_call_uses[0];
        row.shared_cohort = vec![TerminalReborrowSharedCohortMember {
            child_owner_identity: row.child_owner_identity.clone(),
            child_owner_path: row.child_owner_path.clone(),
            child_place: row.child_place.clone(),
            child_access: row.child_access,
            child_activation: row.child_activation.clone(),
            child_weakening: row.child_weakening.clone(),
        }];
    }
    let encoded_shared = encode_module(&shared).expect("sole shared freeze should encode");
    assert_eq!(decode_module(&encoded_shared), Ok(shared.clone()));
    assert_ne!(
        semantic_fingerprint(&shared).expect("shared restoration fingerprints"),
        original
    );

    let mut two_member_shared = shared;
    let mut sibling = two_member_shared.reborrow_restored_call_uses[0].shared_cohort[0].clone();
    sibling.child_owner_identity = borrow_identity('8');
    sibling.child_activation = statement(2);
    two_member_shared.reborrow_restored_call_uses[0]
        .shared_cohort
        .push(sibling);
    {
        let row = &mut two_member_shared.reborrow_restored_call_uses[0];
        row.call_boundary = TerminalBorrowBoundarySource::Call {
            statement_index: 4,
            call_ordinal: 0,
            target_identity: borrow_identity('f'),
        };
        row.child_weakening = statement(4);
        row.direct_root_weakening = statement(6);
        for member in &mut row.shared_cohort {
            member.child_weakening = statement(4);
        }
    }
    let observer = id(3, MachineId::new);
    let structural_type = two_member_shared.structural_types[0].id;
    let caller_place = two_member_shared.machines[0].structural_parameters[0].place;
    two_member_shared.machines[0].blocks[0].operations.insert(
        0,
        Operation {
            id: id(2, OperationId::new),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
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
    let observer_left_place = id(3, PlaceId::new);
    let observer_right_place = id(4, PlaceId::new);
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
    two_member_shared.machines.push(TerminalMachine {
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
    let encoded_two_member =
        encode_module(&two_member_shared).expect("two-member shared freeze should encode");
    assert_eq!(
        decode_module(&encoded_two_member),
        Ok(two_member_shared.clone())
    );
    assert_ne!(
        semantic_fingerprint(&two_member_shared)
            .expect("two-member shared restoration fingerprints"),
        original
    );

    let mut different_target = restored_call_use_module();
    let TerminalBorrowBoundarySource::Call {
        target_identity, ..
    } = &mut different_target.reborrow_restored_call_uses[0].call_boundary
    else {
        unreachable!()
    };
    *target_identity = borrow_identity('9');
    assert_ne!(
        semantic_fingerprint(&different_target).expect("call target fingerprints"),
        original
    );
}

#[test]
fn restored_call_use_encoding_fails_closed_on_substitution() {
    let mut module = restored_call_use_module();
    module.reborrow_restored_call_uses[0].direct_root_lifetime_identity = borrow_identity('f');
    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            ModuleError::InvalidReborrowRestoredCallUse { .. }
        ))
    ));

    let mut wrong_class = restored_call_use_module();
    wrong_class.reborrow_restored_call_uses[0].restoration_class =
        TerminalReborrowRestorationClass::SharedFreezeRestoration;
    assert!(matches!(
        encode_module(&wrong_class),
        Err(CodecError::InvalidModule(
            ModuleError::InvalidReborrowRestoredCallUse { .. }
        ))
    ));

    let mut wrong_statement = restored_call_use_module();
    let TerminalBorrowBoundarySource::Call {
        statement_index, ..
    } = &mut wrong_statement.reborrow_restored_call_uses[0].call_boundary
    else {
        unreachable!()
    };
    *statement_index = 1;
    assert!(matches!(
        encode_module(&wrong_statement),
        Err(CodecError::InvalidModule(
            ModuleError::InvalidReborrowRestoredCallUse { .. }
        ))
    ));

    let mut wrong_ordinal = restored_call_use_module();
    let TerminalBorrowBoundarySource::Call { call_ordinal, .. } =
        &mut wrong_ordinal.reborrow_restored_call_uses[0].call_boundary
    else {
        unreachable!()
    };
    *call_ordinal = 1;
    assert!(matches!(
        encode_module(&wrong_ordinal),
        Err(CodecError::InvalidModule(
            ModuleError::InvalidReborrowRestoredCallUse { .. }
        ))
    ));

    let mut wrong_target_machine = restored_call_use_module();
    wrong_target_machine.reborrow_restored_call_uses[0].call_target_machine = id(3, MachineId::new);
    assert!(matches!(
        encode_module(&wrong_target_machine),
        Err(CodecError::InvalidModule(
            ModuleError::InvalidReborrowRestoredCallUse { .. }
        ))
    ));

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
    let mut missing_member = shared.clone();
    missing_member.reborrow_restored_call_uses[0]
        .shared_cohort
        .clear();
    let mut duplicate_member = shared.clone();
    let member = duplicate_member.reborrow_restored_call_uses[0].shared_cohort[0].clone();
    duplicate_member.reborrow_restored_call_uses[0]
        .shared_cohort
        .push(member);
    let mut retargeted_member = shared;
    retargeted_member.reborrow_restored_call_uses[0].shared_cohort[0].child_owner_identity =
        borrow_identity('9');
    for malformed in [missing_member, duplicate_member, retargeted_member] {
        assert!(matches!(
            encode_module(&malformed),
            Err(CodecError::InvalidModule(
                ModuleError::InvalidReborrowRestoredCallUse { .. }
            ))
        ));
    }
}
