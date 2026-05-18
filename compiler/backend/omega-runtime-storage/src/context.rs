use omega_checked_trees::Program;
use omega_control_flow::ControlFlowPlan;
use omega_layout::LayoutPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_state_storage::StateStoragePlan;
use omega_target::NativeTarget;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStorageContext {
    pub program: Arc<Program>,
    pub control_flow: Arc<ControlFlowPlan>,
    pub layouts: LayoutPlan,
    pub runtime_bodies: RuntimeDispatchBodyPlan,
    pub state_storage: StateStoragePlan,
    pub target: NativeTarget,
}

impl RuntimeStorageContext {
    pub fn new(
        program: Arc<Program>,
        control_flow: Arc<ControlFlowPlan>,
        layouts: &LayoutPlan,
        runtime_bodies: &RuntimeDispatchBodyPlan,
        state_storage: &StateStoragePlan,
        target: NativeTarget,
    ) -> Self {
        Self {
            program,
            control_flow,
            layouts: layouts.clone(),
            runtime_bodies: runtime_bodies.clone(),
            state_storage: state_storage.clone(),
            target,
        }
    }
}
