mod branching;

pub use branching::{
    RuntimeBranchCallExpansion, RuntimeBranchTargetLowering, RuntimeBranchingCall,
    RuntimeBranchingCallEdge, RuntimeBranchingCallPlan, RuntimeLeafBranchBinding,
    RuntimeLeafBranchBindingKind, RuntimeLeafBranchExpansion, RuntimeLeafBranchOperation,
    RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchBinding,
    RuntimeStraightLineBranchBindingKind, RuntimeStraightLineBranchExpansion,
    RuntimeStraightLineBranchOperation, RuntimeStraightLineBranchOperationKind,
    build_runtime_branching_call_plan,
};
use omega_control_flow::ControlFlowPlan;
use omega_platform_interface::HostCallPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_state_calls::StateCallPlan;
use omega_state_storage::StateStoragePlan;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeBranchingContext {
    pub control_flow: ControlFlowPlan,
    pub host_calls: HostCallPlan,
    pub runtime_bodies: RuntimeDispatchBodyPlan,
    pub state_calls: StateCallPlan,
    pub state_storage: StateStoragePlan,
}
