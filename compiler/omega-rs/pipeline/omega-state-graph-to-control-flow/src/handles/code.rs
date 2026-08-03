use omega_control_flow::{
    ContainedFlow, MachineOwnedDataFlow, Operation, StateFlow, StateParameterFlow, TransitionFlow,
};
use psi_arena::HandleSpan;

use super::remap_span;

pub(crate) fn remap_contained_span(
    contained: HandleSpan<omega_state_graph::ContainedGraph>,
) -> HandleSpan<ContainedFlow> {
    remap_span(contained)
}

pub(crate) fn remap_owned_data_span(
    owned_data: HandleSpan<omega_state_graph::MachineOwnedDataGraph>,
) -> HandleSpan<MachineOwnedDataFlow> {
    remap_span(owned_data)
}

pub(crate) fn remap_parameter_span(
    parameters: HandleSpan<omega_state_graph::StateParameterNode>,
) -> HandleSpan<StateParameterFlow> {
    remap_span(parameters)
}

pub(crate) fn remap_state_span(
    states: HandleSpan<omega_state_graph::StateNode>,
) -> HandleSpan<StateFlow> {
    remap_span(states)
}

pub(crate) fn remap_operation_span(
    operations: HandleSpan<omega_state_graph::Operation>,
) -> HandleSpan<Operation> {
    remap_span(operations)
}

pub(crate) fn remap_transition_span(
    transitions: HandleSpan<omega_state_graph::TransitionEdge>,
) -> HandleSpan<TransitionFlow> {
    remap_span(transitions)
}

pub(crate) fn remap_expression_span(
    expressions: HandleSpan<psi_checked_trees::expression::ExpressionHandle>,
) -> HandleSpan<psi_checked_trees::expression::ExpressionHandle> {
    remap_span(expressions)
}
