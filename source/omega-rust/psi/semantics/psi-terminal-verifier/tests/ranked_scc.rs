use psi_core::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_terminal::{
    Block, MachineContract, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, TerminalModule, TerminalRankedGuard, TerminalRankedScc,
    TerminalRankedSccEdge, TerminalRankedSuccessorArgument, Terminator, ValueDeclaration,
    VocabularyMarker,
};
use psi_terminal_verifier::{ModuleError, validate_module, validate_module_representation};

fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero fixture identity")
}

fn ranked_countdown() -> TerminalModule {
    let machine = id(1, MachineId::new);
    let preheader = id(1, BlockId::new);
    let header = id(2, BlockId::new);
    let decrement = id(3, BlockId::new);
    let done = id(4, BlockId::new);
    let initial = id(1, ValueId::new);
    let rank = id(2, ValueId::new);
    let zero = id(3, ValueId::new);
    let condition = id(4, ValueId::new);
    let one = id(5, ValueId::new);
    let next = id(6, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let scalar = ScalarType::Integer(integer);
    let preheader_edge = id(1, EdgeId::new);
    let guard_edge = id(2, EdgeId::new);
    let exit_edge = id(3, EdgeId::new);
    let backedge = id(4, EdgeId::new);
    let return_edge = id(5, EdgeId::new);

    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
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
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: vec![ValueDeclaration {
                id: initial,
                scalar_type: scalar,
            }],
            structural_parameters: Vec::new(),
            ranked_scc: Some(TerminalRankedScc {
                header,
                rank_parameter: rank,
                rank_type: integer,
                lower_bound: IntegerValue::Unsigned(0),
                upper_bound: IntegerValue::Unsigned(u128::from(u32::MAX)),
                covered_cyclic_edges: vec![TerminalRankedSccEdge {
                    edge: backedge,
                    source: decrement,
                    target: header,
                    guard: TerminalRankedGuard::UnsignedParameterPositive {
                        block: header,
                        edge: guard_edge,
                        condition,
                        parameter: rank,
                    },
                    successor_argument:
                        TerminalRankedSuccessorArgument::UnsignedParameterMinusOne {
                            argument_index: 0,
                            argument: next,
                            source_parameter: rank,
                            target_parameter: rank,
                        },
                }],
            }),
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: preheader,
            blocks: vec![
                Block {
                    id: preheader,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge: preheader_edge,
                        target: header,
                        arguments: vec![initial],
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: header,
                    parameters: vec![ValueDeclaration {
                        id: rank,
                        scalar_type: scalar,
                    }],
                    operations: vec![
                        Operation {
                            id: id(1, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                id: zero,
                                scalar_type: scalar,
                            }),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(0),
                            },
                        },
                        Operation {
                            id: id(2, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
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
                            edge: guard_edge,
                            target: decrement,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: exit_edge,
                            target: done,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: decrement,
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            id: id(3, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                id: one,
                                scalar_type: scalar,
                            }),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(1),
                            },
                        },
                        Operation {
                            id: id(4, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                id: next,
                                scalar_type: scalar,
                            }),
                            kind: OperationKind::ExactIntegerSubtract {
                                left: rank,
                                right: one,
                                obligation: id(1, ObligationId::new),
                            },
                        },
                    ],
                    terminator: Terminator::Jump {
                        edge: backedge,
                        target: header,
                        arguments: vec![next],
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: done,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: return_edge,
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: id(1, ContractId::new),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn add_loop_preserved_affine_parameter(module: &mut TerminalModule) -> PlaceId {
    let structural_type = id(1, StructuralTypeId::new);
    let place = id(1, PlaceId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "LoopCustody".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    });
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut machine.blocks[3].terminator
    else {
        panic!("countdown exit must return Unit")
    };
    trivial_affine_discards.push(place);
    place
}

#[test]
fn ranked_countdown_is_representation_only() {
    let module = ranked_countdown();
    assert_eq!(validate_module_representation(&module), Ok(()));
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::NonExecutableRankedScc(machine)) if machine == module.entry
    ));
}

#[test]
fn ranked_countdown_preserves_a_nonempty_structural_frontier() {
    let mut module = ranked_countdown();
    add_loop_preserved_affine_parameter(&mut module);
    assert_eq!(validate_module_representation(&module), Ok(()));
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::NonExecutableRankedScc(machine)) if machine == module.entry
    ));
}

#[test]
fn ranked_countdown_rejects_a_cycle_body_that_changes_structural_custody() {
    let mut module = ranked_countdown();
    let place = add_loop_preserved_affine_parameter(&mut module);
    let machine = &mut module.machines[0];
    let header = machine.ranked_scc.as_ref().unwrap().header;
    let Terminator::Conditional { when_true, .. } = &mut machine.blocks[1].terminator else {
        panic!("countdown header must select the cycle path")
    };
    when_true.trivial_affine_discards.push(place);
    assert_eq!(
        validate_module_representation(&module),
        Err(ModuleError::OwnedStructuralFrontierJoinMismatch(header))
    );
}

#[test]
fn ranked_countdown_rejects_uncovered_or_false_arithmetic() {
    let module = ranked_countdown();
    let mut uncovered = module.clone();
    uncovered.machines[0].ranked_scc = None;
    assert!(matches!(
        validate_module_representation(&uncovered),
        Err(ModuleError::ControlCycle(_))
    ));

    let mut forwards_original = module.clone();
    let rank = forwards_original.machines[0]
        .ranked_scc
        .as_ref()
        .unwrap()
        .rank_parameter;
    let decrement = &mut forwards_original.machines[0].blocks[2];
    let Terminator::Jump { arguments, .. } = &mut decrement.terminator else {
        panic!("decrement backedge")
    };
    arguments[0] = rank;
    assert!(matches!(
        validate_module_representation(&forwards_original),
        Err(ModuleError::InvalidRankedScc(_))
    ));

    let mut wrong_guard = module;
    wrong_guard.machines[0].blocks[1].operations[0].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(1),
    };
    assert!(matches!(
        validate_module_representation(&wrong_guard),
        Err(ModuleError::InvalidRankedScc(_))
    ));
}
