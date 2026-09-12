//! Explicit retired wire vocabulary; current source lowering emits Natural ranks.
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, ScalarType, ValueId,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, SuccessorEdge,
    TerminalMachine, TerminalMachineResult, TerminalModule, TerminalRankedGuard, TerminalRankedScc,
    TerminalRankedSccEdge, TerminalRankedSuccessorArgument, Terminator, ValueDeclaration,
    VocabularyMarker,
};
fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero fixture identity")
}
pub(super) fn legacy_countdown() -> TerminalModule {
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
        scalar_qualifications: Default::default(),
        scalar_range_invariants: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
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
            id: machine,
            attachment: None,
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: initial,
                scalar_type: scalar,
            }],
            structural_parameters: Vec::new(),
            ranked_scc: Some(TerminalRankedScc::UnsignedCountdown(
                terminal_psi::TerminalUnsignedCountdownScc {
                    header,
                    rank_parameter: rank,
                    rank_type: integer,
                    lower_bound: IntegerValue::Unsigned(0),
                    upper_bound: integer.maximum_value(),
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
                },
            )),
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
                    structural_parameters: Vec::new(),
                    id: preheader,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        structural_arguments: Vec::new(),
                        edge: preheader_edge,
                        target: header,
                        arguments: vec![initial],
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    structural_parameters: Vec::new(),
                    id: header,
                    parameters: vec![ValueDeclaration {
                        qualifications: Default::default(),
                        id: rank,
                        scalar_type: scalar,
                    }],
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            id: id(1, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
                                id: zero,
                                scalar_type: scalar,
                            }),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(0),
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            id: id(2, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
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
                            structural_arguments: Vec::new(),
                            edge: guard_edge,
                            target: decrement,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge: exit_edge,
                            target: done,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    structural_parameters: Vec::new(),
                    id: decrement,
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            id: id(3, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
                                id: one,
                                scalar_type: scalar,
                            }),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(1),
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            id: id(4, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
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
                        structural_arguments: Vec::new(),
                        edge: backedge,
                        target: header,
                        arguments: vec![next],
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    structural_parameters: Vec::new(),
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
