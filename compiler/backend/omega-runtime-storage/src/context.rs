use omega_control_flow::ControlFlowPlan;
use omega_layout::LayoutPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_state_storage::StateStoragePlan;
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStorageContext {
    pub control_flow: ControlFlowPlan,
    pub layouts: LayoutPlan,
    pub runtime_bodies: RuntimeDispatchBodyPlan,
    pub state_storage: StateStoragePlan,
    pub target: NativeTarget,
}

impl RuntimeStorageContext {
    pub fn new(
        control_flow: &ControlFlowPlan,
        layouts: &LayoutPlan,
        runtime_bodies: &RuntimeDispatchBodyPlan,
        state_storage: &StateStoragePlan,
        target: NativeTarget,
    ) -> Self {
        Self {
            control_flow: control_flow.clone(),
            layouts: layouts.clone(),
            runtime_bodies: runtime_bodies.clone(),
            state_storage: state_storage.clone(),
            target,
        }
    }
}
