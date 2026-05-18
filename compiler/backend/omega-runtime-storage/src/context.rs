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
    pub layouts: Arc<LayoutPlan>,
    pub runtime_bodies: Arc<RuntimeDispatchBodyPlan>,
    pub state_storage: Arc<StateStoragePlan>,
    pub target: NativeTarget,
}

impl RuntimeStorageContext {
    pub fn new(
        program: Arc<Program>,
        control_flow: Arc<ControlFlowPlan>,
        layouts: Arc<LayoutPlan>,
        runtime_bodies: Arc<RuntimeDispatchBodyPlan>,
        state_storage: Arc<StateStoragePlan>,
        target: NativeTarget,
    ) -> Self {
        Self {
            program,
            control_flow,
            layouts,
            runtime_bodies,
            state_storage,
            target,
        }
    }
}
