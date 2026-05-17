use omega_checked_trees::Program;
use omega_control_flow::ControlFlowPlan;
use omega_platform_interface::HostCallPlan;
use omega_state_calls::StateCallPlan;
use omega_state_dispatch::StateDispatchPlan;
use omega_state_storage::StateStoragePlan;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchBodyContext {
    pub program: Program,
    pub control_flow: ControlFlowPlan,
    pub host_calls: HostCallPlan,
    pub state_dispatch: StateDispatchPlan,
    pub state_calls: StateCallPlan,
    pub state_storage: StateStoragePlan,
}

impl RuntimeDispatchBodyContext {
    pub fn new(
        program: &Program,
        control_flow: &ControlFlowPlan,
        host_calls: &HostCallPlan,
        state_dispatch: &StateDispatchPlan,
        state_calls: &StateCallPlan,
        state_storage: &StateStoragePlan,
    ) -> Self {
        Self {
            program: program.clone(),
            control_flow: control_flow.clone(),
            host_calls: host_calls.clone(),
            state_dispatch: state_dispatch.clone(),
            state_calls: state_calls.clone(),
            state_storage: state_storage.clone(),
        }
    }
}
