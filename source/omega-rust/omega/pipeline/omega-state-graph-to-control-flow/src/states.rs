use omega_control_flow::{StateFlow, StateParameterFlow};
use omega_state_graph::{StateGraph, StateNode, StateParameterNode};
use psi_arena::Arena;

use crate::borrows::remap_borrow_summary;
use crate::boundaries::remap_boundary_summary;
use crate::contracts::remap_contract_summary;
use crate::handles::{remap_operation_span, remap_parameter_span, remap_transition_span};
use crate::ownership::remap_ownership_summary;
use crate::transitions::remap_state_key;
use crate::values::remap_value_summary;

pub(crate) fn remap_states(
    state_graph: &StateGraph,
) -> (Arena<StateFlow>, Arena<StateParameterFlow>) {
    let mut states = Arena::with_capacity(state_graph.states.len());
    let mut state_parameters = Arena::with_capacity(state_graph.state_parameters.len());

    for (_, state) in state_graph.states.iter() {
        states.append(remap_state(state_graph, state, &mut state_parameters));
    }

    (states, state_parameters)
}

pub(crate) fn remap_state_owned(state: StateNode) -> StateFlow {
    StateFlow {
        key: remap_state_key(state.key),
        name: state.name,
        index: state.index,
        service_reach: state.service_reach,
        suspension: state.suspension,
        blocking: state.blocking,
        parameters: remap_parameter_span(state.parameters),
        contracts: remap_contract_summary(&state.contracts),
        values: remap_value_summary(&state.values),
        boundaries: remap_boundary_summary(&state.boundaries),
        borrow: remap_borrow_summary(&state.borrow),
        ownership: remap_ownership_summary(&state.ownership),
        operations: remap_operation_span(state.operations),
        transitions: remap_transition_span(state.transitions),
    }
}

pub(crate) fn remap_parameter_owned(parameter: StateParameterNode) -> StateParameterFlow {
    StateParameterFlow {
        symbol: parameter.symbol,
        name: parameter.name,
        type_reference: parameter.type_reference,
        type_symbol: parameter.type_symbol,
        type_name: parameter.type_name,
        is_mutable_reference: parameter.is_mutable_reference,
        dyn_conformance_candidates: parameter.dyn_conformance_candidates,
        dyn_conformance_rows: parameter.dyn_conformance_rows,
    }
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
        service_reach: state.service_reach,
        suspension: state.suspension,
        blocking: state.blocking,
        parameters: state_parameters.insert_many(
            state_graph
                .state_parameters(state)
                .iter()
                .map(remap_parameter),
        ),
        contracts: remap_contract_summary(&state.contracts),
        values: remap_value_summary(&state.values),
        boundaries: remap_boundary_summary(&state.boundaries),
        borrow: remap_borrow_summary(&state.borrow),
        ownership: remap_ownership_summary(&state.ownership),
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
        dyn_conformance_candidates: parameter.dyn_conformance_candidates.clone(),
        dyn_conformance_rows: parameter.dyn_conformance_rows.clone(),
    }
}
