mod builder;
mod model;

use crate::control_flow::{ControlFlowPlan, StateKey};
use omega_core::diagnostics::Diagnostic;

use builder::RuntimeFlowBuilder;
pub use model::{
    RuntimeCycle, RuntimeEdge, RuntimeFlowPlan, RuntimeState, RuntimeTransitionTarget,
};

pub fn build_runtime_flow_plan(
    control_flow: &ControlFlowPlan,
    entry_key: StateKey,
) -> Result<RuntimeFlowPlan, Diagnostic> {
    let entry_machine_flow = control_flow
        .machine_by_symbol(entry_key.machine)
        .ok_or_else(|| Diagnostic::error("unknown runtime entry machine"))?;
    let entry_state_flow = control_flow
        .state_by_key(entry_key)
        .ok_or_else(|| Diagnostic::error("unknown runtime entry state"))?;
    let mut builder = RuntimeFlowBuilder::new(control_flow);

    builder.visit_state(RuntimeState {
        key: entry_key,
        machine: entry_machine_flow.name.clone(),
        state: entry_state_flow.name.clone(),
    })?;

    Ok(builder.finish())
}
