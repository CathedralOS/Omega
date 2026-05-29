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
            contract_fact_refs: semantics
                .contract_fact_refs
                .map(remap_contract_fact_ref_owned),
            contract_calls: semantics.contract_calls.map(remap_contract_call_owned),
            contract_exits: semantics.contract_exits.map(remap_contract_exit_owned),
            values: semantics.values.map(remap_value_owned),
            boundary_edges: semantics.boundary_edges.map(remap_boundary_edge_owned),
            borrow_writable_roots: semantics
                .borrow_writable_roots
                .map(remap_borrow_writable_root_owned),
            borrow_access_segments: semantics.borrow_access_segments,
            borrow_argument_accesses: semantics
                .borrow_argument_accesses
                .map(remap_borrow_argument_access_owned),
            borrow_calls: semantics.borrow_calls.map(remap_borrow_call_owned),
            borrow_loans: semantics.borrow_loans.map(remap_borrow_loan_owned),
            borrow_activations: semantics
                .borrow_activations
                .map(remap_borrow_activation_owned),
            borrow_weakenings: semantics
                .borrow_weakenings
                .map(remap_borrow_weakening_owned),
            ownership_segments: semantics.ownership_segments,
            move_events: semantics.move_events.map(remap_move_event_owned),
            drop_events: semantics.drop_events.map(remap_drop_event_owned),
        },
    })
}
