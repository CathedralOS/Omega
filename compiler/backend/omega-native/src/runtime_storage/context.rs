use crate::plan::NativePlan;
use crate::state_storage::StateStoragePlan;
use crate::target::NativeTarget;
use omega_layout::LayoutPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStorageContext {
    pub layouts: LayoutPlan,
    pub state_storage: StateStoragePlan,
    pub target: NativeTarget,
}

impl RuntimeStorageContext {
    pub fn from_native_plan(native_plan: &NativePlan) -> Self {
        Self {
            layouts: native_plan.layouts.clone(),
            state_storage: native_plan.state_storage.clone(),
            target: native_plan.target,
        }
    }
}
