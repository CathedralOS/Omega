mod branching;

pub use branching::{
    RuntimeBranchCallExpansion, RuntimeBranchPreludeBinding, RuntimeBranchPreludeExpansion,
    RuntimeBranchPreludeOperation, RuntimeBranchPreludeOperationKind, RuntimeBranchTargetLowering,
    RuntimeBranchingCall, RuntimeBranchingCallEdge, RuntimeBranchingCallPlan,
    RuntimeLeafBranchBinding, RuntimeLeafBranchBindingKind, RuntimeLeafBranchExpansion,
    RuntimeLeafBranchOperation, RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchBinding,
    RuntimeStraightLineBranchBindingKind, RuntimeStraightLineBranchExpansion,
    RuntimeStraightLineBranchOperation, RuntimeStraightLineBranchOperationKind,
    build_runtime_branching_call_plan,
};
use omega_control_flow::ControlFlowPlan;
use omega_platform_interface::HostCallPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_state_calls::StateCallPlan;
use omega_state_dispatch::StateDispatchPlan;
use omega_state_guards::StateGuardPlan;
use omega_state_storage::StateStoragePlan;

#[derive(Debug, Clone, Copy)]
pub struct RuntimeBranchingContext<'plan> {
    pub program: &'plan psi_checked_trees::CheckedTrees,
    pub control_flow: &'plan ControlFlowPlan,
    pub host_calls: &'plan HostCallPlan,
    pub runtime_bodies: &'plan RuntimeDispatchBodyPlan,
    pub state_calls: &'plan StateCallPlan,
    pub state_dispatch: &'plan StateDispatchPlan,
    pub state_guards: &'plan StateGuardPlan,
    pub state_storage: &'plan StateStoragePlan,
}
