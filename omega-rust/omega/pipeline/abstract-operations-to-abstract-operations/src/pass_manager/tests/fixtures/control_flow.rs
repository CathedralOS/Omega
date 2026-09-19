//! Control-flow-shaped verified fixtures: linear threading, a merge
//! parameter, and a conditional that admits no cleanup.

use super::super::VerifiedPsiOptimizationUnit;
use super::admission::verified_unit;

fn empty_contract(id: u64) -> terminal_psi::MachineContract {
    use semantic_vocabulary::ContractId;
    terminal_psi::MachineContract {
        erased_scalar_formals: Vec::new(),
        id: ContractId::new(id).unwrap(),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

fn module(machine: terminal_psi::TerminalMachine) -> terminal_psi::TerminalModule {
    use terminal_psi::VocabularyMarker;
    terminal_psi::TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine.id,
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
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![machine],
    }
}

fn machine(
    id: u64,
    parameters: Vec<terminal_psi::ValueDeclaration>,
    result: terminal_psi::TerminalMachineResult,
    entry: semantic_vocabulary::BlockId,
    blocks: Vec<terminal_psi::Block>,
    contract: u64,
) -> terminal_psi::TerminalMachine {
    use semantic_vocabulary::MachineId;
    terminal_psi::TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(id).unwrap(),
        attachment: None,
        parameters,
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks,
        contract: empty_contract(contract),
    }
}

/// A single-predecessor empty block threads entry bindings into the target:
/// `entry -[c1,c2]-> empty(a,b) -[b,a]-> target(x,y) -[]-> return unit`.
pub(in crate::pass_manager::tests) fn verified_linear_empty_block_unit()
-> VerifiedPsiOptimizationUnit {
    use semantic_vocabulary::{
        BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
    };
    use terminal_psi::{
        Block, Operation, OperationKind, OperationResult, TerminalMachineResult, Terminator,
        ValueDeclaration,
    };

    let (entry, empty, target) = (
        BlockId::new(5_202).unwrap(),
        BlockId::new(5_203).unwrap(),
        BlockId::new(5_204).unwrap(),
    );
    let (c1, c2) = (ValueId::new(5_205).unwrap(), ValueId::new(5_206).unwrap());
    let (a, b) = (ValueId::new(5_207).unwrap(), ValueId::new(5_208).unwrap());
    let (x, y) = (ValueId::new(5_209).unwrap(), ValueId::new(5_210).unwrap());
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let declaration = |id| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    };
    let jump = |edge, target, arguments| Terminator::Jump {
        erased_arguments: Vec::new(),
        edge,
        target,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    verified_unit(
        &module(machine(
            5_201,
            Vec::new(),
            TerminalMachineResult::Unit,
            entry,
            vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: entry,
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            id: OperationId::new(5_211).unwrap(),
                            result: OperationResult::Scalar(declaration(c1)),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(3),
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            id: OperationId::new(5_212).unwrap(),
                            result: OperationResult::Scalar(declaration(c2)),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(4),
                            },
                        },
                    ],
                    terminator: jump(EdgeId::new(5_213).unwrap(), empty, vec![c1, c2]),
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: empty,
                    parameters: vec![declaration(a), declaration(b)],
                    operations: Vec::new(),
                    terminator: jump(EdgeId::new(5_214).unwrap(), target, vec![b, a]),
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: target,
                    parameters: vec![declaration(x), declaration(y)],
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(5_215).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            5_299,
        )),
        &terminal_verifier::ProofBundle::default(),
    )
}

/// A conditional binds the merge block's parameter to the same source on both
/// arms (redundant) or to a distinct source on the false arm (boundary).
pub(in crate::pass_manager::tests) fn verified_merge_parameter_unit(
    redundant: bool,
) -> VerifiedPsiOptimizationUnit {
    use semantic_vocabulary::{
        BlockId, EdgeId, IntegerSign, IntegerType, OperationId, ScalarType, ValueId,
    };
    use terminal_psi::{
        Block, Operation, OperationKind, OperationResult, SuccessorEdge, TerminalMachineResult,
        Terminator, ValueDeclaration,
    };

    let (entry, merge) = (BlockId::new(5_302).unwrap(), BlockId::new(5_303).unwrap());
    let condition = ValueId::new(5_304).unwrap();
    let shared = ValueId::new(5_305).unwrap();
    let alternate = ValueId::new(5_306).unwrap();
    let parameter = ValueId::new(5_307).unwrap();
    let sum = ValueId::new(5_308).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let declaration = |id, scalar_type| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    };
    let successor = |edge: u64, arguments| SuccessorEdge {
        erased_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target: merge,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    verified_unit(
        &module(machine(
            5_301,
            vec![
                declaration(condition, ScalarType::Boolean),
                declaration(shared, scalar_type),
                declaration(alternate, scalar_type),
            ],
            TerminalMachineResult::Scalar(declaration(ValueId::new(5_312).unwrap(), scalar_type)),
            entry,
            vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: entry,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: successor(5_309, vec![shared]),
                        when_false: successor(
                            5_310,
                            vec![if redundant { shared } else { alternate }],
                        ),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: merge,
                    parameters: vec![declaration(parameter, scalar_type)],
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: OperationId::new(5_311).unwrap(),
                        result: OperationResult::Scalar(declaration(sum, scalar_type)),
                        kind: OperationKind::WrappingIntegerAdd {
                            left: parameter,
                            right: alternate,
                        },
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(5_313).unwrap(),
                        value: sum,
                    },
                },
            ],
            5_399,
        )),
        &terminal_verifier::ProofBundle::default(),
    )
}

/// A conditional whose two arms return unit through distinct blocks: every
/// cleanup rule sees a real join and declines.
pub(in crate::pass_manager::tests) fn verified_conditional_distinct_returns_unit()
-> VerifiedPsiOptimizationUnit {
    use semantic_vocabulary::{BlockId, EdgeId, ScalarType, ValueId};
    use terminal_psi::{Block, SuccessorEdge, TerminalMachineResult, Terminator, ValueDeclaration};

    let (entry, when_true, when_false) = (
        BlockId::new(5_402).unwrap(),
        BlockId::new(5_403).unwrap(),
        BlockId::new(5_404).unwrap(),
    );
    let condition = ValueId::new(5_405).unwrap();
    let successor = |edge: u64, target| SuccessorEdge {
        erased_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let return_unit = |edge| Terminator::ReturnUnit {
        edge,
        trivial_affine_discards: Vec::new(),
    };
    verified_unit(
        &module(machine(
            5_401,
            vec![ValueDeclaration {
                qualifications: Default::default(),
                id: condition,
                scalar_type: ScalarType::Boolean,
            }],
            TerminalMachineResult::Unit,
            entry,
            vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: entry,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: successor(5_406, when_true),
                        when_false: successor(5_407, when_false),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: when_true,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: return_unit(EdgeId::new(5_408).unwrap()),
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: when_false,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: return_unit(EdgeId::new(5_409).unwrap()),
                },
            ],
            5_499,
        )),
        &terminal_verifier::ProofBundle::default(),
    )
}
