use omega_control_flow::{ControlFlowPlan, StateFlow, StateParameterFlow};
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_state_graph::{StateGraph, StateNode, StateParameterNode};

use crate::borrows::{
    remap_borrow_activation_owned, remap_borrow_activations, remap_borrow_argument_access_owned,
    remap_borrow_argument_accesses, remap_borrow_call_owned, remap_borrow_calls,
    remap_borrow_loan_owned, remap_borrow_loans, remap_borrow_summary,
    remap_borrow_weakening_owned, remap_borrow_weakenings, remap_borrow_writable_root_owned,
    remap_borrow_writable_roots,
};
use crate::contracts::{
    remap_contract_call_owned, remap_contract_calls, remap_contract_exit_owned,
    remap_contract_exits, remap_contract_fact_ref_owned, remap_contract_fact_refs,
    remap_contract_summary,
};
use crate::facts::{
    remap_invariant_owned, remap_invariants, remap_proof_obligation_owned, remap_proof_obligations,
};
use crate::handles::{remap_operation_span, remap_parameter_span, remap_transition_span};
use crate::machines::{
    remap_contained_owned, remap_machine_owned, remap_machines, remap_owned_data_owned,
};
use crate::operations::{remap_operation_owned, remap_operations};
use crate::transitions::{remap_state_key, remap_transition_owned, remap_transitions};

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
        borrow_writable_roots: remap_borrow_writable_roots(state_graph),
        borrow_access_segments: state_graph.borrow_access_segments.clone(),
        borrow_argument_accesses: remap_borrow_argument_accesses(state_graph),
        borrow_calls: remap_borrow_calls(state_graph),
        borrow_loans: remap_borrow_loans(state_graph),
        borrow_activations: remap_borrow_activations(state_graph),
        borrow_weakenings: remap_borrow_weakenings(state_graph),
        operations: remap_operations(state_graph),
        transitions: remap_transitions(state_graph),
    })
}

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
        borrow_writable_roots,
        borrow_access_segments,
        borrow_argument_accesses,
        borrow_calls,
        borrow_loans,
        borrow_activations,
        borrow_weakenings,
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
        proof_obligations: proof_obligations.map(remap_proof_obligation_owned),
        invariants: invariants.map(remap_invariant_owned),
        contract_fact_refs: contract_fact_refs.map(remap_contract_fact_ref_owned),
        contract_calls: contract_calls.map(remap_contract_call_owned),
        contract_exits: contract_exits.map(remap_contract_exit_owned),
        borrow_writable_roots: borrow_writable_roots.map(remap_borrow_writable_root_owned),
        borrow_access_segments,
        borrow_argument_accesses: borrow_argument_accesses.map(remap_borrow_argument_access_owned),
        borrow_calls: borrow_calls.map(remap_borrow_call_owned),
        borrow_loans: borrow_loans.map(remap_borrow_loan_owned),
        borrow_activations: borrow_activations.map(remap_borrow_activation_owned),
        borrow_weakenings: borrow_weakenings.map(remap_borrow_weakening_owned),
        operations: operations.map(remap_operation_owned),
        transitions: transitions.map(remap_transition_owned),
    })
}

fn remap_states(state_graph: &StateGraph) -> (Arena<StateFlow>, Arena<StateParameterFlow>) {
    let mut states = Arena::with_capacity(state_graph.states.len());
    let mut state_parameters = Arena::with_capacity(state_graph.state_parameters.len());

    for (_, state) in state_graph.states.iter() {
        states.append(remap_state(state_graph, state, &mut state_parameters));
    }

    (states, state_parameters)
}

fn remap_state(
    state_graph: &StateGraph,
    state: &StateNode,
    state_parameters: &mut Arena<StateParameterFlow>,
) -> StateFlow {
    StateFlow {
        key: remap_state_key(state.key),
        name: state.name.clone(),
        index: state.index,
        direct_effects: state.direct_effects,
        reached_effects: state.reached_effects,
        parameters: state_parameters.insert_many(
            state_graph
                .state_parameters(state)
                .iter()
                .map(remap_parameter),
        ),
        contracts: remap_contract_summary(&state.contracts),
        borrow: remap_borrow_summary(&state.borrow),
        operations: remap_operation_span(state.operations),
        transitions: remap_transition_span(state.transitions),
    }
}

fn remap_state_owned(state: StateNode) -> StateFlow {
    StateFlow {
        key: remap_state_key(state.key),
        name: state.name,
        index: state.index,
        direct_effects: state.direct_effects,
        reached_effects: state.reached_effects,
        parameters: remap_parameter_span(state.parameters),
        contracts: remap_contract_summary(&state.contracts),
        borrow: remap_borrow_summary(&state.borrow),
        operations: remap_operation_span(state.operations),
        transitions: remap_transition_span(state.transitions),
    }
}

fn remap_parameter(parameter: &StateParameterNode) -> StateParameterFlow {
    StateParameterFlow {
        symbol: parameter.symbol,
        name: parameter.name.clone(),
        type_reference: parameter.type_reference,
        type_symbol: parameter.type_symbol,
        type_name: parameter.type_name.clone(),
        is_mutable_reference: parameter.is_mutable_reference,
    }
}

fn remap_parameter_owned(parameter: StateParameterNode) -> StateParameterFlow {
    StateParameterFlow {
        symbol: parameter.symbol,
        name: parameter.name,
        type_reference: parameter.type_reference,
        type_symbol: parameter.type_symbol,
        type_name: parameter.type_name,
        is_mutable_reference: parameter.is_mutable_reference,
    }
}
