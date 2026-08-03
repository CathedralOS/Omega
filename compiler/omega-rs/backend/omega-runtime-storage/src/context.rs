use omega_control_flow::ControlFlowPlan;
use omega_layout::LayoutPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_state_calls::StateCallPlan;
use omega_state_storage::StateStoragePlan;
use omega_target::NativeTarget;
use psi_checked_trees::CheckedTrees;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStorageContext {
    pub program: Arc<CheckedTrees>,
    pub control_flow: Arc<ControlFlowPlan>,
    pub layouts: Arc<LayoutPlan>,
    pub runtime_bodies: Arc<RuntimeDispatchBodyPlan>,
    pub state_calls: Arc<StateCallPlan>,
    pub state_storage: Arc<StateStoragePlan>,
    pub target: NativeTarget,
}

impl RuntimeStorageContext {
    pub fn new(
        program: Arc<CheckedTrees>,
        control_flow: Arc<ControlFlowPlan>,
        layouts: Arc<LayoutPlan>,
        runtime_bodies: Arc<RuntimeDispatchBodyPlan>,
        state_calls: Arc<StateCallPlan>,
        state_storage: Arc<StateStoragePlan>,
        target: NativeTarget,
    ) -> Self {
        Self {
            program,
            control_flow,
            layouts,
            runtime_bodies,
            state_calls,
            state_storage,
            target,
        }
    }
}
