use super::inputs::dispatch_index_for_key;
use omega_control_flow::StateKey;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_state_dispatch::StateDispatchPlan;
use omega_state_guards::StateGuardPlan;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchLoopContext {
    pub(super) needed: bool,
    pub(super) entry_dispatch_index: u32,
    pub(super) state_guards: StateGuardPlan,
    pub(super) runtime_bodies: RuntimeDispatchBodyPlan,
}

impl RuntimeDispatchLoopContext {
    pub fn from_parts(
        needed: bool,
        state_dispatch: &StateDispatchPlan,
        entry_key: StateKey,
        state_guards: StateGuardPlan,
        runtime_bodies: RuntimeDispatchBodyPlan,
    ) -> Self {
        Self {
            needed,
            entry_dispatch_index: dispatch_index_for_key(state_dispatch, entry_key),
            state_guards,
            runtime_bodies,
        }
    }
}
