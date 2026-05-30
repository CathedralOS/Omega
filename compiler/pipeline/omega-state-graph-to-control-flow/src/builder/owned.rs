use omega_control_flow::ControlFlowPlan;
use omega_core::diagnostics::Diagnostic;
use omega_state_graph::StateGraph;

use crate::code::remap_code_roots_owned;
use crate::semantics::remap_semantic_roots_owned;

pub(crate) fn build_control_flow_plan_owned(
    state_graph: StateGraph,
) -> Result<ControlFlowPlan, Diagnostic> {
    let StateGraph { code, semantics } = state_graph;

    Ok(ControlFlowPlan {
        code: remap_code_roots_owned(code),
        semantics: remap_semantic_roots_owned(semantics),
    })
}
