use crate::plan::NativePlan;
use crate::state_storage::StateStoragePlan;
use omega_control_flow::ControlFlowPlan;
use omega_layout::LayoutPlan;
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStorageContext {
    pub control_flow: ControlFlowPlan,
    pub layouts: LayoutPlan,
    pub state_storage: StateStoragePlan,
    pub target: NativeTarget,
}

impl RuntimeStorageContext {
    pub fn from_native_plan(native_plan: &NativePlan) -> Self {
        Self {
            control_flow: native_plan.control_flow.clone(),
            layouts: native_plan.layouts.clone(),
            state_storage: native_plan.state_storage.clone(),
            target: native_plan.target,
        }
    }
}
