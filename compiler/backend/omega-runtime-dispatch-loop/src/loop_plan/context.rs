use super::inputs::dispatch_index_for_key;
use omega_control_flow::StateKey;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_state_dispatch::StateDispatchPlan;
use omega_state_guards::StateGuardPlan;
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchLoopContext {
    pub(super) needed: bool,
    pub(super) entry_dispatch_index: u32,
    pub(super) state_dispatch: Arc<StateDispatchPlan>,
    pub(super) state_guards: Arc<StateGuardPlan>,
    pub(super) runtime_bodies: Arc<RuntimeDispatchBodyPlan>,
}

impl RuntimeDispatchLoopContext {
    pub fn from_parts(
        needed: bool,
        state_dispatch: Arc<StateDispatchPlan>,
        entry_key: StateKey,
        state_guards: Arc<StateGuardPlan>,
        runtime_bodies: Arc<RuntimeDispatchBodyPlan>,
    ) -> Self {
        let entry_dispatch_index = dispatch_index_for_key(&state_dispatch, entry_key);

        Self {
            needed,
            entry_dispatch_index,
            state_guards,
            state_dispatch,
            runtime_bodies,
        }
    }
}
