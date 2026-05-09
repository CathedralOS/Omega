mod builder;

use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_core::diagnostics::Diagnostic;
use omega_state_graph::{RuntimeFlowPlan, RuntimeState};

use builder::RuntimeFlowBuilder;

pub fn build_runtime_flow_plan(
    control_flow: &ControlFlowPlan,
    entry_key: StateKey,
) -> Result<RuntimeFlowPlan, Diagnostic> {
    control_flow
        .machine_by_symbol(entry_key.machine)
        .ok_or_else(|| Diagnostic::error("unknown runtime entry machine"))?;
    control_flow
        .state_by_key(entry_key)
        .ok_or_else(|| Diagnostic::error("unknown runtime entry state"))?;
    let mut builder = RuntimeFlowBuilder::new(control_flow);

    builder.visit_state(RuntimeState { key: entry_key })?;

    Ok(builder.finish())
}
