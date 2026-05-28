use omega_control_flow::ControlFlowPlan;
use omega_core::diagnostics::Diagnostic;
use omega_state_graph::StateGraph;

mod borrows;
mod builder;
mod contracts;
mod facts;
mod handles;
mod machines;
mod operations;
mod ownership;
mod states;
mod transitions;

pub fn build_control_flow_plan(state_graph: &StateGraph) -> Result<ControlFlowPlan, Diagnostic> {
    builder::build_control_flow_plan(state_graph)
}

pub fn build_control_flow_plan_owned(
    state_graph: StateGraph,
) -> Result<ControlFlowPlan, Diagnostic> {
    builder::build_control_flow_plan_owned(state_graph)
}
