//! State-specialization verified fixtures: a dispatch state whose incoming
//! edges supply a mixed constant/non-constant state argument, and a dispatch
//! whose every incoming edge is constant and must decline.

use super::super::VerifiedPsiOptimizationUnit;
use super::admission::verified_unit;
use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId, ScalarType, ValueId};
use terminal_psi::{
    Block, Operation, OperationKind, OperationResult, SuccessorEdge, TerminalMachineResult,
    Terminator, ValueDeclaration,
};

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

fn boolean(id: u64) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(id).unwrap(),
        scalar_type: ScalarType::Boolean,
    }
}

fn return_unit(edge: u64) -> Terminator {
    Terminator::ReturnUnit {
        edge: EdgeId::new(edge).unwrap(),
        trivial_affine_discards: Vec::new(),
    }
}

/// A single-`Conditional` dispatch block parameter is bound by two incoming
/// unconditional edges: `pred_true` supplies the proven literal `flag`, while
/// `pred_false` supplies the machine parameter `seed`, which no sparse
/// constant lattice can prove. Exactly the constant-supplied edge specializes.
///
/// `entry -[seed]-> {pred_true, pred_false} -> dispatch(state) -> {yes, no}`.
pub(in crate::pass_manager::tests) fn verified_dispatch_specialization_unit()
-> VerifiedPsiOptimizationUnit {
    let (entry, pred_true, pred_false, dispatch, yes, no) = (
        BlockId::new(5_602).unwrap(),
        BlockId::new(5_603).unwrap(),
        BlockId::new(5_604).unwrap(),
        BlockId::new(5_605).unwrap(),
        BlockId::new(5_606).unwrap(),
        BlockId::new(5_607).unwrap(),
    );
    let seed = ValueId::new(5_608).unwrap();
    let flag = ValueId::new(5_609).unwrap();
    let state = ValueId::new(5_610).unwrap();
    let successor = |edge: u64, target, arguments| SuccessorEdge {
        erased_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let jump = |edge: u64, arguments| Terminator::Jump {
        erased_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target: dispatch,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    verified_unit(
        &module(terminal_psi::TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(5_601).unwrap(),
            attachment: None,
            parameters: vec![boolean(5_608)],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            contract: empty_contract(5_699),
            blocks: vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: entry,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: OperationId::new(5_611).unwrap(),
                        result: OperationResult::Scalar(boolean(5_609)),
                        kind: OperationKind::BooleanConstant { value: true },
                    }],
                    terminator: Terminator::Conditional {
                        condition: seed,
                        when_true: successor(5_612, pred_true, Vec::new()),
                        when_false: successor(5_613, pred_false, Vec::new()),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: pred_true,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: jump(5_614, vec![flag]),
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: pred_false,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: jump(5_615, vec![seed]),
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: dispatch,
                    parameters: vec![boolean(5_610)],
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition: state,
                        when_true: successor(5_616, yes, Vec::new()),
                        when_false: successor(5_617, no, Vec::new()),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: yes,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: return_unit(5_618),
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: no,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: return_unit(5_619),
                },
            ],
        }),
        &terminal_verifier::ProofBundle::default(),
    )
}

/// Every incoming edge of `dispatch` supplies a proven Boolean literal, so the
/// specialization would orphan the state and the pass must decline — the
/// boundary leg at this family's exact admission edge.
pub(in crate::pass_manager::tests) fn verified_dispatch_all_constant_unit()
-> VerifiedPsiOptimizationUnit {
    let (entry, dispatch, yes, no) = (
        BlockId::new(5_702).unwrap(),
        BlockId::new(5_703).unwrap(),
        BlockId::new(5_704).unwrap(),
        BlockId::new(5_705).unwrap(),
    );
    let flag = ValueId::new(5_706).unwrap();
    let state = ValueId::new(5_707).unwrap();
    let successor = |edge: u64, target| SuccessorEdge {
        erased_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    verified_unit(
        &module(terminal_psi::TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(5_701).unwrap(),
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
            entry,
            contract: empty_contract(5_799),
            blocks: vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: entry,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: OperationId::new(5_708).unwrap(),
                        result: OperationResult::Scalar(boolean(5_706)),
                        kind: OperationKind::BooleanConstant { value: true },
                    }],
                    terminator: Terminator::Jump {
                        erased_arguments: Vec::new(),
                        edge: EdgeId::new(5_709).unwrap(),
                        target: dispatch,
                        arguments: vec![flag],
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: dispatch,
                    parameters: vec![boolean(5_707)],
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition: state,
                        when_true: successor(5_710, yes),
                        when_false: successor(5_711, no),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: yes,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: return_unit(5_712),
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: no,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: return_unit(5_713),
                },
            ],
        }),
        &terminal_verifier::ProofBundle::default(),
    )
}
