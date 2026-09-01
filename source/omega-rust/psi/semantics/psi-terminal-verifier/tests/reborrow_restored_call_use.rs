use psi_core::{
    BlockId, ContractId, EdgeId, MachineId, OperationId, PlaceId, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use psi_terminal::{
    Block, MachineContract, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalBorrowBoundarySource, TerminalBorrowPlace, TerminalMachine,
    TerminalMachineResult, TerminalModule, TerminalReborrowRestoredCallUse, Terminator,
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
    validate_module(&restored_call_use_module()).expect("exact restored-parent call use");
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
        module.reborrow_restored_call_uses[0].child_access = StructuralAccess::SharedBorrow;
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

    let mut reordered = restored_call_use_module();
    let mut second_operation = reordered.machines[0].blocks[0].operations[0].clone();
    second_operation.id = id(2, OperationId::new);
    reordered.machines[0].blocks[0]
        .operations
        .push(second_operation);
    let mut second_row = reordered.reborrow_restored_call_uses[0].clone();
    second_row.operation = id(2, OperationId::new);
    reordered.reborrow_restored_call_uses.push(second_row);
    reordered.reborrow_restored_call_uses.reverse();
    assert!(matches!(
        validate_module(&reordered),
        Err(ModuleError::NonCanonicalReborrowRestoredCallUseOrder)
    ));
}
