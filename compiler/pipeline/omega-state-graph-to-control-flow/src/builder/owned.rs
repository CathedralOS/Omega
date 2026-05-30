use omega_control_flow::{ControlFlowCode, ControlFlowPlan};
use omega_core::diagnostics::Diagnostic;
use omega_state_graph::{StateGraph, StateGraphCode};

use crate::borrows::{
    remap_borrow_activation_owned, remap_borrow_argument_access_owned, remap_borrow_call_owned,
    remap_borrow_loan_owned, remap_borrow_weakening_owned, remap_borrow_writable_root_owned,
};
use crate::boundaries::remap_boundary_edge_owned;
use crate::contracts::{
    remap_contract_call_owned, remap_contract_exit_owned, remap_contract_fact_ref_owned,
};
use crate::facts::{remap_invariant_owned, remap_proof_obligation_owned};
use crate::machines::{remap_contained_owned, remap_machine_owned, remap_owned_data_owned};
use crate::operations::remap_operation_owned;
use crate::ownership::{remap_drop_event_owned, remap_move_event_owned};
use crate::states::{remap_parameter_owned, remap_state_owned};
use crate::transitions::remap_transition_owned;
use crate::values::remap_value_owned;

pub(crate) fn build_control_flow_plan_owned(
    state_graph: StateGraph,
) -> Result<ControlFlowPlan, Diagnostic> {
    let StateGraph { code, semantics } = state_graph;
    let StateGraphCode {
        expressions,
        machines,
        contained_machines,
        machine_owned_data,
        states,
        state_parameters,
        operations,
        transitions,
    } = code;

    Ok(ControlFlowPlan {
        code: ControlFlowCode {
            expressions,
            machines: machines.map(remap_machine_owned),
            contained_machines: contained_machines.map(remap_contained_owned),
            machine_owned_data: machine_owned_data.map(remap_owned_data_owned),
            states: states.map(remap_state_owned),
            state_parameters: state_parameters.map(remap_parameter_owned),
            operations: operations.map(remap_operation_owned),
            transitions: transitions.map(remap_transition_owned),
        },
        semantics: omega_control_flow::ControlFlowSemanticRoots {
            proof_obligations: semantics
                .proof_obligations
                .map(remap_proof_obligation_owned),
            invariants: semantics.invariants.map(remap_invariant_owned),
            contracts: omega_control_flow::ControlFlowContractRoots {
                fact_refs: semantics
                    .contracts
                    .fact_refs
                    .map(remap_contract_fact_ref_owned),
                calls: semantics.contracts.calls.map(remap_contract_call_owned),
                exits: semantics.contracts.exits.map(remap_contract_exit_owned),
            },
            values: semantics.values.map(remap_value_owned),
            boundary_edges: semantics.boundary_edges.map(remap_boundary_edge_owned),
            borrow: omega_control_flow::ControlFlowBorrowRoots {
                writable_roots: semantics
                    .borrow
                    .writable_roots
                    .map(remap_borrow_writable_root_owned),
                access_segments: semantics.borrow.access_segments,
                argument_accesses: semantics
                    .borrow
                    .argument_accesses
                    .map(remap_borrow_argument_access_owned),
                calls: semantics.borrow.calls.map(remap_borrow_call_owned),
                loans: semantics.borrow.loans.map(remap_borrow_loan_owned),
                activations: semantics
                    .borrow
                    .activations
                    .map(remap_borrow_activation_owned),
                weakenings: semantics
                    .borrow
                    .weakenings
                    .map(remap_borrow_weakening_owned),
            },
            ownership: omega_control_flow::ControlFlowOwnershipRoots {
                segments: semantics.ownership.segments,
                moves: semantics.ownership.moves.map(remap_move_event_owned),
                drops: semantics.ownership.drops.map(remap_drop_event_owned),
            },
        },
    })
}
