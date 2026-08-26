use omega_abstract_operations::{AbstractDataPlan, AbstractOperationPlan};
use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_backend_plan::{
    BackendArtifactRoots, BackendPlan, BackendPlanPhaseTiming, BoundNominalCallbackPlacement,
    CallbackPrivateRelocationDemand, CallbackThunkPlan,
};
use omega_calling_conventions::{BoundaryEntryPlan, HostAbiPlan};
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_layout::LayoutPlan;
use omega_platform_interface::HostCallPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_runtime_branching::RuntimeBranchingCallPlan;
use omega_runtime_dispatch_loop::RuntimeDispatchLoopPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use omega_runtime_text::RuntimeTextPlan;
use omega_state_calls::{AliasFlowPlan, StateCallPlan};
use omega_state_dispatch::StateDispatchPlan;
use omega_state_graph::RuntimeFlowPlan;
use omega_state_guards::StateGuardPlan;
use omega_state_storage::StateStoragePlan;
use omega_state_values::StateValuePlan;
use omega_target::{NativeTarget, TargetProfile};
use omega_target_operations::{TargetDataPlan, TargetOperationPlan};
use psi_arena::Arena;
use std::sync::Arc;

pub(super) struct BackendPlanSkeletonInput {
    pub target_profile: TargetProfile,
    pub target: NativeTarget,
    pub host_abi: Arc<HostAbiPlan>,
    pub host_calls: HostCallPlan,
    pub control_flow: Arc<ControlFlowPlan>,
    pub runtime_flow: Arc<RuntimeFlowPlan>,
    pub state_dispatch: StateDispatchPlan,
    pub state_guards: StateGuardPlan,
    pub layouts: LayoutPlan,
    pub entry_key: StateKey,
    pub entry_boundary_plan: Option<BoundaryEntryPlan>,
    pub callback_placements: Arc<[BoundNominalCallbackPlacement]>,
    pub callback_thunks: Arc<[CallbackThunkPlan]>,
    pub callback_private_relocations: Arc<[CallbackPrivateRelocationDemand]>,
    pub phase_timings: Arena<BackendPlanPhaseTiming>,
}

pub(super) fn build_backend_plan_skeleton(input: BackendPlanSkeletonInput) -> BackendPlan {
    BackendPlan {
        target_profile: input.target_profile,
        target: input.target,
        artifacts: BackendArtifactRoots::empty_for_target(input.target),
        host_abi: input.host_abi,
        host_calls: Arc::new(input.host_calls),
        state_calls: Arc::new(StateCallPlan::default()),
        alias_flow: AliasFlowPlan::default(),
        state_storage: Arc::new(StateStoragePlan::default()),
        state_values: StateValuePlan::default(),
        abstract_data: AbstractDataPlan::default(),
        data: TargetDataPlan::default(),
        abstract_operations: AbstractOperationPlan::default(),
        target_operations: TargetOperationPlan::default(),
        assigned_target_operations: AssignedTargetOperationPlan::default(),
        control_flow: input.control_flow,
        runtime_flow: input.runtime_flow,
        state_dispatch: Arc::new(input.state_dispatch),
        state_guards: Arc::new(input.state_guards),
        runtime_bodies: Arc::new(RuntimeDispatchBodyPlan::default()),
        runtime_branching_calls: RuntimeBranchingCallPlan::default(),
        runtime_dispatch_loop: RuntimeDispatchLoopPlan::default(),
        runtime_storage: RuntimeStoragePlan::default(),
        runtime_text: RuntimeTextPlan::default(),
        layouts: Arc::new(input.layouts),
        entry_key: input.entry_key,
        entry_boundary_plan: input.entry_boundary_plan,
        callback_placements: input.callback_placements,
        callback_thunks: input.callback_thunks,
        callback_private_relocations: input.callback_private_relocations,
        callback_registrar_arguments: Arc::from([]),
        callback_registrar_destinations: Arc::from([]),
        receiver_bases: Vec::new(),
        state_contexts: Vec::new(),
        phase_timings: input.phase_timings,
    }
}
