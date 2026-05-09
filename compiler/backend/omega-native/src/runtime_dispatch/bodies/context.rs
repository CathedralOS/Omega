use crate::plan::NativePlan;
use omega_control_flow::ControlFlowPlan;
use omega_platform_interface::HostCallPlan;
use omega_state_storage::StateStoragePlan;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchBodyContext {
    pub control_flow: ControlFlowPlan,
    pub host_calls: HostCallPlan,
    pub state_calls: omega_state_calls::StateCallPlan,
    pub state_storage: StateStoragePlan,
}

impl RuntimeDispatchBodyContext {
    pub fn from_native_plan(native_plan: &NativePlan) -> Self {
        Self {
            control_flow: native_plan.control_flow.clone(),
            host_calls: native_plan.host_calls.clone(),
            state_calls: native_plan.state_calls.clone(),
            state_storage: native_plan.state_storage.clone(),
        }
    }
}
