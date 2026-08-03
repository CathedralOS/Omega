use omega_control_flow::ControlFlowPlan;
use omega_state_graph::StateGraph;
use psi_diagnostics::Diagnostic;

use crate::code::remap_code_roots_owned;
use crate::semantics::remap_semantic_roots_owned;

pub(crate) fn build_control_flow_plan_owned(
    state_graph: StateGraph,
) -> Result<ControlFlowPlan, Diagnostic> {
    let StateGraph { code, semantics } = state_graph;

    Ok(ControlFlowPlan::with_roots(
        remap_code_roots_owned(code),
        remap_semantic_roots_owned(semantics),
    ))
}
