use omega_control_flow::ControlFlowPlan;
use omega_core::diagnostics::Diagnostic;
use omega_state_graph::StateGraph;

use crate::borrows::{
    remap_borrow_activations, remap_borrow_argument_accesses, remap_borrow_calls,
    remap_borrow_loans, remap_borrow_weakenings, remap_borrow_writable_roots,
};
use crate::boundaries::remap_boundary_edges;
use crate::contracts::{remap_contract_calls, remap_contract_exits, remap_contract_fact_refs};
use crate::facts::{remap_invariants, remap_proof_obligations};
use crate::machines::remap_machines;
use crate::operations::remap_operations;
use crate::ownership::{remap_drop_events, remap_move_events};
use crate::states::remap_states;
use crate::transitions::remap_transitions;
use crate::values::remap_values;

pub(crate) fn build_control_flow_plan(
    state_graph: &StateGraph,
) -> Result<ControlFlowPlan, Diagnostic> {
    let (machines, contained_machines, machine_owned_data) = remap_machines(state_graph);
    let (states, state_parameters) = remap_states(state_graph);

    Ok(ControlFlowPlan {
        expressions: state_graph.expressions.clone(),
        machines,
        contained_machines,
        machine_owned_data,
        states,
        state_parameters,
        proof_obligations: remap_proof_obligations(state_graph),
        invariants: remap_invariants(state_graph),
        contract_fact_refs: remap_contract_fact_refs(state_graph),
        contract_calls: remap_contract_calls(state_graph),
        contract_exits: remap_contract_exits(state_graph),
        values: remap_values(state_graph),
        boundary_edges: remap_boundary_edges(state_graph),
        borrow_writable_roots: remap_borrow_writable_roots(state_graph),
        borrow_access_segments: state_graph.borrow_access_segments.clone(),
        borrow_argument_accesses: remap_borrow_argument_accesses(state_graph),
        borrow_calls: remap_borrow_calls(state_graph),
        borrow_loans: remap_borrow_loans(state_graph),
        borrow_activations: remap_borrow_activations(state_graph),
        borrow_weakenings: remap_borrow_weakenings(state_graph),
        ownership_segments: state_graph.ownership_segments.clone(),
        move_events: remap_move_events(state_graph),
        drop_events: remap_drop_events(state_graph),
        operations: remap_operations(state_graph),
        transitions: remap_transitions(state_graph),
    })
}
