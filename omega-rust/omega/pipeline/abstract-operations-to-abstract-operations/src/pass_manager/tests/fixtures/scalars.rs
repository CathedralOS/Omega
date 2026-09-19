//! Scalar-shaped verified fixtures: dead literals and a parameter add.

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

fn module(
    machine: u64,
    parameters: Vec<terminal_psi::ValueDeclaration>,
    result: terminal_psi::TerminalMachineResult,
    entry: semantic_vocabulary::BlockId,
    blocks: Vec<terminal_psi::Block>,
) -> terminal_psi::TerminalModule {
    use semantic_vocabulary::MachineId;
    use terminal_psi::VocabularyMarker;
    terminal_psi::TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(machine).unwrap(),
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
        machines: vec![terminal_psi::TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(machine).unwrap(),
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
            contract: empty_contract(5_999),
        }],
    }
}

/// Two unused literal results feed a unit return: pure dead scalar work.
pub(in crate::pass_manager::tests) fn verified_dead_literals_unit() -> VerifiedPsiOptimizationUnit {
    use semantic_vocabulary::{
        BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
    };
    use terminal_psi::{
        Block, Operation, OperationKind, OperationResult, TerminalMachineResult, Terminator,
        ValueDeclaration,
    };

    let block = BlockId::new(5_102).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let declaration = |id, scalar_type| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    };
    verified_unit(
        &module(
            5_101,
            Vec::new(),
            TerminalMachineResult::Unit,
            block,
            vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: block,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        static_reach_binding: None,
                        id: OperationId::new(5_103).unwrap(),
                        result: OperationResult::Scalar(declaration(
                            ValueId::new(5_104).unwrap(),
                            ScalarType::Boolean,
                        )),
                        kind: OperationKind::BooleanConstant { value: true },
                    },
                    Operation {
                        static_reach_binding: None,
                        id: OperationId::new(5_105).unwrap(),
                        result: OperationResult::Scalar(declaration(
                            ValueId::new(5_106).unwrap(),
                            scalar_type,
                        )),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(7),
                        },
                    },
                ],
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(5_107).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
        ),
        &terminal_verifier::ProofBundle::default(),
    )
}

/// One dead literal beside one returned literal: the use boundary inside a
/// single block.
pub(in crate::pass_manager::tests) fn verified_half_dead_literals_unit()
-> VerifiedPsiOptimizationUnit {
    use semantic_vocabulary::{
        BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
    };
    use terminal_psi::{
        Block, Operation, OperationKind, OperationResult, TerminalMachineResult, Terminator,
        ValueDeclaration,
    };

    let block = BlockId::new(5_112).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let live = ValueId::new(5_116).unwrap();
    let declaration = |id| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    };
    verified_unit(
        &module(
            5_111,
            Vec::new(),
            TerminalMachineResult::Scalar(declaration(ValueId::new(5_118).unwrap())),
            block,
            vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: block,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        static_reach_binding: None,
                        id: OperationId::new(5_113).unwrap(),
                        result: OperationResult::Scalar(declaration(ValueId::new(5_114).unwrap())),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(7),
                        },
                    },
                    Operation {
                        static_reach_binding: None,
                        id: OperationId::new(5_115).unwrap(),
                        result: OperationResult::Scalar(declaration(live)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(9),
                        },
                    },
                ],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(5_117).unwrap(),
                    value: live,
                },
            }],
        ),
        &terminal_verifier::ProofBundle::default(),
    )
}

/// Two machine parameters feed a wrapping add: the operands stay unknown, so
/// constant propagation can evaluate but never fold.
pub(in crate::pass_manager::tests) fn verified_parameter_add_unit() -> VerifiedPsiOptimizationUnit {
    use semantic_vocabulary::{
        BlockId, EdgeId, IntegerSign, IntegerType, OperationId, ScalarType, ValueId,
    };
    use terminal_psi::{
        Block, Operation, OperationKind, OperationResult, TerminalMachineResult, Terminator,
        ValueDeclaration,
    };

    let machine = 5_121;
    let block = BlockId::new(5_122).unwrap();
    let left = ValueId::new(5_123).unwrap();
    let right = ValueId::new(5_124).unwrap();
    let sum = ValueId::new(5_126).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let declaration = |id| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    };
    verified_unit(
        &module(
            machine,
            vec![declaration(left), declaration(right)],
            TerminalMachineResult::Scalar(declaration(ValueId::new(5_128).unwrap())),
            block,
            vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: block,
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(5_125).unwrap(),
                    result: OperationResult::Scalar(declaration(sum)),
                    kind: OperationKind::WrappingIntegerAdd { left, right },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(5_127).unwrap(),
                    value: sum,
                },
            }],
        ),
        &terminal_verifier::ProofBundle::default(),
    )
}
