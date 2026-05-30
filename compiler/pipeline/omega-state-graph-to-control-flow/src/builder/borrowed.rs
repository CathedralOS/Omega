use omega_control_flow::{ControlFlowCode, ControlFlowPlan};
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
        code: ControlFlowCode {
            expressions: state_graph.expressions.clone(),
            machines,
            contained_machines,
            machine_owned_data,
            states,
            state_parameters,
            operations: remap_operations(state_graph),
            transitions: remap_transitions(state_graph),
        },
        semantics: omega_control_flow::ControlFlowSemanticRoots {
            proof_obligations: remap_proof_obligations(state_graph),
            invariants: remap_invariants(state_graph),
            contracts: omega_control_flow::ControlFlowContractRoots {
                fact_refs: remap_contract_fact_refs(state_graph),
                calls: remap_contract_calls(state_graph),
                exits: remap_contract_exits(state_graph),
            },
            values: remap_values(state_graph),
            boundary_edges: remap_boundary_edges(state_graph),
            borrow: omega_control_flow::ControlFlowBorrowRoots {
                writable_roots: remap_borrow_writable_roots(state_graph),
                access_segments: state_graph.semantics.borrow.access_segments.clone(),
                argument_accesses: remap_borrow_argument_accesses(state_graph),
                calls: remap_borrow_calls(state_graph),
                loans: remap_borrow_loans(state_graph),
                activations: remap_borrow_activations(state_graph),
                weakenings: remap_borrow_weakenings(state_graph),
            },
            ownership: omega_control_flow::ControlFlowOwnershipRoots {
                segments: state_graph.semantics.ownership.segments.clone(),
                moves: remap_move_events(state_graph),
                drops: remap_drop_events(state_graph),
            },
        },
    })
}
