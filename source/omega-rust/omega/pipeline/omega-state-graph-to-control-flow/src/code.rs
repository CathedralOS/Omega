use omega_control_flow::{ControlFlowCode, StateParameterFlow};
use omega_state_graph::{StateGraph, StateGraphCode};
use psi_arena::Arena;

use crate::machines::{
    remap_contained_owned, remap_machine_owned, remap_machines, remap_owned_data_owned,
};
use crate::operations::{remap_operation_owned, remap_operations};
use crate::states::{remap_parameter_owned, remap_state_owned, remap_states};
use crate::transitions::{remap_transition_owned, remap_transitions};

pub(crate) fn remap_code_roots(state_graph: &StateGraph) -> ControlFlowCode {
    let (machines, contained_machines, machine_owned_data) = remap_machines(state_graph);
    let (states, state_parameters) = remap_states(state_graph);

    ControlFlowCode::with_roots(
        state_graph.expressions.clone(),
        machines,
        contained_machines,
        machine_owned_data,
        states,
        state_parameters,
        remap_operations(state_graph),
        remap_transitions(state_graph),
    )
}

pub(crate) fn remap_code_roots_owned(code: StateGraphCode) -> ControlFlowCode {
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

    ControlFlowCode::with_roots(
        expressions,
        machines.map(remap_machine_owned),
        contained_machines.map(remap_contained_owned),
        machine_owned_data.map(remap_owned_data_owned),
        states.map(remap_state_owned),
        remap_parameters_owned(state_parameters),
        operations.map(remap_operation_owned),
        transitions.map(remap_transition_owned),
    )
}

fn remap_parameters_owned(
    parameters: Arena<omega_state_graph::StateParameterNode>,
) -> Arena<StateParameterFlow> {
    parameters.map(remap_parameter_owned)
}
