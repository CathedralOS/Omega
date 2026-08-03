use omega_control_flow::ControlFlowPlan;
use omega_state_graph::StateGraph;
use psi_diagnostics::Diagnostic;

use crate::code::remap_code_roots;
use crate::semantics::remap_semantic_roots;

pub(crate) fn build_control_flow_plan(
    state_graph: &StateGraph,
) -> Result<ControlFlowPlan, Diagnostic> {
    Ok(ControlFlowPlan::with_roots(
        remap_code_roots(state_graph),
        remap_semantic_roots(state_graph),
    ))
}
