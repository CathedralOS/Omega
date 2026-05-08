use super::inputs::dispatch_index_for_key;
use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyPlan;
use crate::state_guards::StateGuardPlan;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchLoopContext {
    pub(super) needed: bool,
    pub(super) entry_dispatch_index: u32,
    pub(super) state_guards: StateGuardPlan,
    pub(super) runtime_bodies: RuntimeDispatchBodyPlan,
}

impl RuntimeDispatchLoopContext {
    pub fn from_native_plan(native_plan: &NativePlan) -> Self {
        Self {
            needed: !native_plan.runtime_flow.cycles.is_empty(),
            entry_dispatch_index: dispatch_index_for_key(
                &native_plan.state_dispatch,
                native_plan.entry_key,
            ),
            state_guards: native_plan.state_guards.clone(),
            runtime_bodies: native_plan.runtime_bodies.clone(),
        }
    }
}
