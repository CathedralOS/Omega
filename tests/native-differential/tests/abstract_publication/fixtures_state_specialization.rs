//! State-specialization projection fixtures: a dispatch state whose incoming
//! edges supply a mixed constant/non-constant state argument.

use super::{
    Block, BlockId, ContractId, EdgeId, MachineContract, MachineId, Operation, OperationId,
    OperationKind, OperationResult, ProofBundle, ScalarType, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, ValueId,
    VerifiedPsiOptimizationUnit, VocabularyMarker, verified,
};

fn module(machine: TerminalMachine) -> TerminalModule {
    TerminalModule {
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
pub(super) fn state_specialization_dispatch_verified() -> VerifiedPsiOptimizationUnit {
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
        erased_proof_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let jump = |edge: u64, arguments| Terminator::Jump {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target: dispatch,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    verified(
        module(TerminalMachine {
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
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                id: ContractId::new(5_699).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
            blocks: vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: entry,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
                        suspension_crossing: None,
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
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: pred_true,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: jump(5_614, vec![flag]),
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: pred_false,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: jump(5_615, vec![seed]),
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
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
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: yes,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: return_unit(5_618),
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: no,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: return_unit(5_619),
                },
            ],
        }),
        ProofBundle::default(),
    )
}
