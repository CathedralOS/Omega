use omega_control_flow::ControlFlowPlan;
use omega_state_graph::StateGraph;
use psi_diagnostics::Diagnostic;

mod arena_remap;
mod borrows;
mod boundaries;
mod builder;
mod code;
mod contracts;
mod facts;
mod handles;
mod machines;
mod operations;
mod ownership;
mod semantics;
mod states;
mod transitions;
mod values;

pub fn build_control_flow_plan(state_graph: &StateGraph) -> Result<ControlFlowPlan, Diagnostic> {
    builder::build_control_flow_plan(state_graph)
}

pub fn build_control_flow_plan_owned(
    state_graph: StateGraph,
) -> Result<ControlFlowPlan, Diagnostic> {
    builder::build_control_flow_plan_owned(state_graph)
}
