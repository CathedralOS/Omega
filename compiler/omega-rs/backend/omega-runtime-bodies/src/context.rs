use omega_control_flow::ControlFlowPlan;
use omega_platform_interface::HostCallPlan;
use omega_state_calls::StateCallPlan;
use omega_state_dispatch::StateDispatchPlan;
use omega_state_storage::StateStoragePlan;
use psi_checked_trees::CheckedTrees;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDispatchBodyContext {
    pub program: Arc<CheckedTrees>,
    pub control_flow: Arc<ControlFlowPlan>,
    pub host_calls: Arc<HostCallPlan>,
    pub state_dispatch: Arc<StateDispatchPlan>,
    pub state_calls: Arc<StateCallPlan>,
    pub state_storage: Arc<StateStoragePlan>,
}

impl RuntimeDispatchBodyContext {
    pub fn new(
        program: Arc<CheckedTrees>,
        control_flow: Arc<ControlFlowPlan>,
        host_calls: Arc<HostCallPlan>,
        state_dispatch: Arc<StateDispatchPlan>,
        state_calls: Arc<StateCallPlan>,
        state_storage: Arc<StateStoragePlan>,
    ) -> Self {
        Self {
            program,
            control_flow,
            host_calls,
            state_dispatch,
            state_calls,
            state_storage,
        }
    }
}
