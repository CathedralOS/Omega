use omega_control_flow::ControlFlowPlan;
use omega_core::diagnostics::Diagnostic;
use omega_state_graph::StateGraph;

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
    let StateGraph {
        expressions,
        machines,
        contained_machines,
        machine_owned_data,
        states,
        state_parameters,
        proof_obligations,
        invariants,
        contract_fact_refs,
        contract_calls,
        contract_exits,
        values,
        boundary_edges,
        borrow_writable_roots,
        borrow_access_segments,
        borrow_argument_accesses,
        borrow_calls,
        borrow_loans,
        borrow_activations,
        borrow_weakenings,
        ownership_segments,
        move_events,
        drop_events,
        operations,
        transitions,
    } = state_graph;

    Ok(ControlFlowPlan {
        expressions,
        machines: machines.map(remap_machine_owned),
        contained_machines: contained_machines.map(remap_contained_owned),
        machine_owned_data: machine_owned_data.map(remap_owned_data_owned),
        states: states.map(remap_state_owned),
        state_parameters: state_parameters.map(remap_parameter_owned),
        semantics: omega_control_flow::ControlFlowSemanticRoots {
            proof_obligations: proof_obligations.map(remap_proof_obligation_owned),
            invariants: invariants.map(remap_invariant_owned),
            contract_fact_refs: contract_fact_refs.map(remap_contract_fact_ref_owned),
            contract_calls: contract_calls.map(remap_contract_call_owned),
            contract_exits: contract_exits.map(remap_contract_exit_owned),
            values: values.map(remap_value_owned),
            boundary_edges: boundary_edges.map(remap_boundary_edge_owned),
            borrow_writable_roots: borrow_writable_roots.map(remap_borrow_writable_root_owned),
            borrow_access_segments,
            borrow_argument_accesses: borrow_argument_accesses
                .map(remap_borrow_argument_access_owned),
            borrow_calls: borrow_calls.map(remap_borrow_call_owned),
            borrow_loans: borrow_loans.map(remap_borrow_loan_owned),
            borrow_activations: borrow_activations.map(remap_borrow_activation_owned),
            borrow_weakenings: borrow_weakenings.map(remap_borrow_weakening_owned),
            ownership_segments,
            move_events: move_events.map(remap_move_event_owned),
            drop_events: drop_events.map(remap_drop_event_owned),
        },
        operations: operations.map(remap_operation_owned),
        transitions: transitions.map(remap_transition_owned),
    })
}
