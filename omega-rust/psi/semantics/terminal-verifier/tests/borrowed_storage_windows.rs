//! Borrowed-storage restoration windows: `MoveStructuralField` opens a
//! restoration debt beneath a mutable-borrowed machine parameter and only an
//! exact `StoreStructuralField` at the same canonical hole closes it. These
//! fixtures replay the checked transform's evidence through Terminal
//! verification: the debt, not the producer's claim, is what every non-crash
//! exit, edge, join, and operation is checked against.

use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, ContractId, EdgeId, MachineId, OperationId, PlaceId,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, Block, CrashCause, MachineContract, Operation, OperationKind,
    OperationResult, RecordFieldValue, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, VocabularyMarker,
};
use terminal_verifier::{ModuleError, validate_module};

fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero fixture identity")
}

fn cell() -> StructuralTypeId {
    id(1, StructuralTypeId::new)
}

fn envelope() -> StructuralTypeId {
    id(2, StructuralTypeId::new)
}

fn flag() -> StructuralFieldId {
    id(1, StructuralFieldId::new)
}

fn left() -> StructuralFieldId {
    id(1, StructuralFieldId::new)
}

fn right() -> StructuralFieldId {
    id(2, StructuralFieldId::new)
}

fn structural_types() -> Vec<StructuralTypeDeclaration> {
    vec![
        StructuralTypeDeclaration {
            id: cell(),
            identity: "Cell".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: flag(),
                    identity: "flag".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(
                        semantic_vocabulary::ScalarType::Boolean,
                    ),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: envelope(),
            identity: "Envelope".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: left(),
                        identity: "left".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(cell()),
                    },
                    StructuralFieldDeclaration {
                        id: right(),
                        identity: "right".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(cell()),
                    },
                ],
            },
        },
    ]
}

fn machine() -> TerminalMachine {
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: id(1, MachineId::new),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: id(1, PlaceId::new),
            position: 0,
            is_self: false,
            structural_type: envelope(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: id(1, PlaceId::new),
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: id(2, PlaceId::new),
                kind: StructuralPlaceKind::OperationResult {
                    producer: id(1, OperationId::new),
                    structural_type: cell(),
                },
            },
        ],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: id(1, BlockId::new),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: id(1, BlockId::new),
            parameters: Vec::new(),
            operations: vec![extract(id(1, OperationId::new), 2), repair(2)],
            terminator: Terminator::ReturnUnit {
                edge: id(1, EdgeId::new),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: id(1, ContractId::new),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

/// `MoveStructuralField` opening the `left` window on the borrowed envelope
/// parameter, producing the moved `Cell` at `result`.
fn extract(operation: OperationId, result: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        id: operation,
        result: OperationResult::Structural(StructuralOperationResult {
            place: id(result, PlaceId::new),
            structural_type: cell(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::MoveStructuralField {
            source: id(1, PlaceId::new),
            path: Vec::new(),
            field: left(),
        },
    }
}

/// `StoreStructuralField` reseating the `left` window with `value`.
fn repair(value: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        id: id(2, OperationId::new),
        result: OperationResult::Unit,
        kind: OperationKind::StoreStructuralField {
            destination: id(1, PlaceId::new),
            path: Vec::new(),
            field: left(),
            value: StructuralArgument {
                place: id(value, PlaceId::new),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
        },
    }
}

/// A fresh `Cell` established whole for `producer`, stored at `place`.
fn fresh_cell(operation: OperationId, place: u64, flag_value: ValueId) -> Operation {
    Operation {
        static_reach_binding: None,
        id: operation,
        result: OperationResult::Structural(StructuralOperationResult {
            place: id(place, PlaceId::new),
            structural_type: cell(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishRecord {
            fields: vec![terminal_psi::RecordFieldInitializer {
                field: flag(),
                value: RecordFieldValue::Scalar {
                    value: flag_value,
                    range_obligation: None,
                },
            }],
        },
    }
}

/// A `BooleanConstant` scalar producer.
fn boolean_constant(operation: OperationId, result: ValueId, value: bool) -> Operation {
    Operation {
        static_reach_binding: None,
        id: operation,
        result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type: semantic_vocabulary::ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value },
    }
}

/// A Boolean field read: canonical `path` walks to the record containing
/// `field` beneath `source`.
fn boolean_read(
    operation: OperationId,
    result: ValueId,
    source: u64,
    path: Vec<CanonicalStructuralPathSegment>,
    field: StructuralFieldId,
) -> Operation {
    Operation {
        static_reach_binding: None,
        id: operation,
        result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type: semantic_vocabulary::ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanStructuralField {
            source: id(source, PlaceId::new),
            path,
            field,
        },
    }
}

fn window_module() -> TerminalModule {
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: id(1, MachineId::new),
        structural_types: structural_types(),
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
        machines: vec![machine()],
    }
}

fn assert_rejects(module: &TerminalModule, expected: ModuleError) {
    assert_eq!(
        validate_module(module).unwrap_err(),
        expected,
        "unexpected validation outcome"
    );
}

#[test]
fn extraction_and_exact_repair_validate() {
    validate_module(&window_module()).expect("move then repair validates");
}

#[test]
fn a_disjoint_sibling_stays_observable() {
    let mut module = window_module();
    let operations = &mut module.machines[0].blocks[0].operations;
    // Between the move and the repair the `right` subtree was never absent:
    // the debt is path-granular, not whole-root.
    operations.insert(
        1,
        boolean_read(
            id(5, OperationId::new),
            id(1, ValueId::new),
            1,
            vec![CanonicalStructuralPathSegment::Field(right())],
            flag(),
        ),
    );
    validate_module(&module).expect("disjoint sibling stays observable");
}

#[test]
fn a_different_same_typed_value_repairs_the_window() {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(3, OperationId::new),
            structural_type: cell(),
        },
    });
    let operations = &mut machine.blocks[0].operations;
    // Repair with a freshly established Cell rather than the extracted one:
    // the debt names the hole and the required type, not the moved identity.
    operations.insert(
        0,
        boolean_constant(id(5, OperationId::new), id(1, ValueId::new), false),
    );
    operations.insert(
        2,
        fresh_cell(id(3, OperationId::new), 3, id(1, ValueId::new)),
    );
    let OperationKind::StoreStructuralField { value, .. } = &mut operations[3].kind else {
        unreachable!()
    };
    value.place = id(3, PlaceId::new);
    validate_module(&module).expect("replacement repair validates");
}

#[test]
fn missing_repair_rejects_at_the_exit() {
    let mut module = window_module();
    module.machines[0].blocks[0].operations.pop();
    assert_rejects(
        &module,
        ModuleError::BorrowedStorageRestorationPending {
            machine: id(1, MachineId::new),
            block: id(1, BlockId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn an_early_return_through_a_second_block_rejects() {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    machine.blocks[0].operations.pop();
    machine.blocks[0].terminator = Terminator::Jump {
        edge: id(2, EdgeId::new),
        target: id(2, BlockId::new),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    machine.blocks.push(Block {
        structural_parameters: Vec::new(),
        id: id(2, BlockId::new),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnUnit {
            edge: id(3, EdgeId::new),
            trivial_affine_discards: Vec::new(),
        },
    });
    assert_rejects(
        &module,
        ModuleError::BorrowedStorageRestorationPending {
            machine: id(1, MachineId::new),
            block: id(2, BlockId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn a_store_at_the_wrong_place_rejects() {
    let mut module = window_module();
    // `right` was never vacated; the only open window is `left`.
    let OperationKind::StoreStructuralField { field, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *field = right();
    assert_rejects(
        &module,
        ModuleError::BorrowedStorageRepairMismatch {
            operation: id(2, OperationId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn a_scalar_field_cannot_open_a_window() {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(5, OperationId::new),
            structural_type: cell(),
        },
    });
    let operations = &mut machine.blocks[0].operations;
    // `flag` is a scalar leaf of the Cell, not a structural subtree; a window
    // can only open on a declared `Structural` field.
    operations.insert(
        1,
        Operation {
            static_reach_binding: None,
            id: id(5, OperationId::new),
            result: OperationResult::Structural(StructuralOperationResult {
                place: id(3, PlaceId::new),
                structural_type: cell(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::MoveStructuralField {
                source: id(1, PlaceId::new),
                path: vec![terminal_psi::StructuralPathSegment::Field("left".into())],
                field: flag(),
            },
        },
    );
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::InvalidBorrowedStorageWindow {
            operation: id(5, OperationId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn a_repair_value_of_the_wrong_type_rejects() {
    let mut module = window_module();
    // A whole owned Envelope is not the Cell the open window requires.
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: id(6, PlaceId::new),
            position: 1,
            is_self: false,
            structural_type: envelope(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(6, PlaceId::new),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let OperationKind::StoreStructuralField { value, .. } =
        &mut machine.blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    value.place = id(6, PlaceId::new);
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::InvalidBorrowedStorageWindow {
            operation: id(2, OperationId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn repeated_extraction_of_the_open_window_rejects() {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(4, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(4, OperationId::new),
            structural_type: cell(),
        },
    });
    machine.blocks[0]
        .operations
        .insert(1, extract(id(4, OperationId::new), 4));
    assert_rejects(
        &module,
        ModuleError::BorrowedStorageFieldAbsent {
            operation: id(4, OperationId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn a_stale_read_under_the_open_window_rejects() {
    let mut module = window_module();
    module.machines[0].blocks[0].operations.insert(
        1,
        boolean_read(
            id(5, OperationId::new),
            id(1, ValueId::new),
            1,
            vec![CanonicalStructuralPathSegment::Field(left())],
            flag(),
        ),
    );
    assert_rejects(
        &module,
        ModuleError::BorrowedStorageFieldAbsent {
            operation: id(5, OperationId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn the_window_cannot_cross_an_edge() {
    for (name, argument, parameter_type) in [
        (
            "the exact absent subtree",
            vec![terminal_psi::StructuralPathSegment::Field("left".into())],
            cell(),
        ),
        ("the whole owner", Vec::new(), envelope()),
    ] {
        let mut module = window_module();
        let machine = &mut module.machines[0];
        machine.blocks[0].operations.pop();
        machine.blocks[0].terminator = Terminator::Jump {
            edge: id(2, EdgeId::new),
            target: id(2, BlockId::new),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: id(1, PlaceId::new),
                path: argument,
                access: StructuralAccess::SharedBorrow,
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        };
        machine.blocks.push(Block {
            structural_parameters: vec![StructuralParameterDeclaration {
                place: id(5, PlaceId::new),
                position: 0,
                is_self: false,
                structural_type: parameter_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            id: id(2, BlockId::new),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: id(3, EdgeId::new),
                trivial_affine_discards: Vec::new(),
            },
        });
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: id(5, PlaceId::new),
            kind: StructuralPlaceKind::BlockParameter {
                block: id(2, BlockId::new),
                position: 0,
            },
        });
        let result = validate_module(&module);
        assert!(
            matches!(
                result,
                Err(ModuleError::InvalidStructuralSuccessorArgument {
                    edge,
                    place,
                }) if edge == id(2, EdgeId::new) && place == id(1, PlaceId::new)
            ),
            "{name} across the edge rejected unexpectedly: {result:?}"
        );
    }
}

#[test]
fn a_join_must_carry_the_window_through_both_arms() {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    // The true arm opens the window; the false arm does not. The merge cannot
    // reconstruct one frontier, so the join itself rejects.
    machine.blocks[0].operations = vec![boolean_constant(
        id(5, OperationId::new),
        id(1, ValueId::new),
        true,
    )];
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: id(1, ValueId::new),
        when_true: SuccessorEdge {
            edge: id(2, EdgeId::new),
            target: id(2, BlockId::new),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: SuccessorEdge {
            edge: id(3, EdgeId::new),
            target: id(3, BlockId::new),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    let open_arm = Block {
        structural_parameters: Vec::new(),
        id: id(2, BlockId::new),
        parameters: Vec::new(),
        operations: vec![extract(id(6, OperationId::new), 2)],
        terminator: Terminator::Jump {
            edge: id(4, EdgeId::new),
            target: id(4, BlockId::new),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    };
    let quiet_arm = Block {
        structural_parameters: Vec::new(),
        id: id(3, BlockId::new),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Jump {
            edge: id(5, EdgeId::new),
            target: id(4, BlockId::new),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    };
    let merge = Block {
        structural_parameters: Vec::new(),
        id: id(4, BlockId::new),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnUnit {
            edge: id(6, EdgeId::new),
            trivial_affine_discards: Vec::new(),
        },
    };
    // The extracted result place must name its producer on the moved arm.
    machine
        .structural_places
        .retain(|place| !matches!(place.kind, StructuralPlaceKind::OperationResult { .. }));
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(2, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(6, OperationId::new),
            structural_type: cell(),
        },
    });
    machine.blocks.append(&mut vec![open_arm, quiet_arm, merge]);
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::OwnedStructuralFrontierJoinMismatch(id(4, BlockId::new)),
    );
}

#[test]
fn a_consumed_affine_value_cannot_repair_twice() {
    let mut module = window_module();
    let operations = &mut module.machines[0].blocks[0].operations;
    // The moved result is affine now; the repair consumes it exactly once.
    let OperationResult::Structural(result) = &mut operations[0].result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    let mut second = repair(2);
    second.id = id(4, OperationId::new);
    operations.push(second);
    // The first store closes the debt and consumes the affine value; the
    // second finds neither a live value nor an open window.
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: id(4, OperationId::new),
            place: id(2, PlaceId::new),
        },
    );
}

#[test]
fn a_repair_to_a_closed_root_rejects() {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(3, OperationId::new),
            structural_type: cell(),
        },
    });
    // The first store has already reseated `left`; a second store to the same
    // field is a replacement-with-cleanup claim this operation never carries.
    machine.blocks[0].operations.insert(
        2,
        fresh_cell(id(3, OperationId::new), 3, id(1, ValueId::new)),
    );
    machine.blocks[0].operations.insert(
        0,
        boolean_constant(id(5, OperationId::new), id(1, ValueId::new), true),
    );
    let mut second = repair(3);
    second.id = id(4, OperationId::new);
    machine.blocks[0].operations.push(second);
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::BorrowedStorageRepairMismatch {
            operation: id(4, OperationId::new),
            place: id(1, PlaceId::new),
        },
    );
}

#[test]
fn crash_abandons_the_window_without_cleanup() {
    let mut module = window_module();
    let machine = &mut module.machines[0];
    machine.blocks[0].operations.pop();
    machine.contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
    }];
    machine.blocks[0].terminator = Terminator::Crash {
        edge: id(2, EdgeId::new),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    validate_module(&module).expect("crash retains no restoration obligation");
}
