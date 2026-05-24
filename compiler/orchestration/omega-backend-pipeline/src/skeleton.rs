use omega_abstract_operations::{AbstractDataPlan, AbstractOperationPlan};
use omega_backend_plan::{BackendPlan, BackendPlanPhaseTiming};
use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_calling_conventions::HostAbiPlan;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_core::arena::{Arena, Handle};
use omega_layout::LayoutPlan;
use omega_machine_bytes::EncodedMachinePlan;
use omega_machine_instructions::MachineInstructionPlan;
use omega_object_file::{ObjectPlan, RelocationPlan, SymbolPlan};
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
use omega_target::NativeTarget;
use omega_target_operations::{TargetDataPlan, TargetOperationPlan};
use std::sync::Arc;

pub(super) struct BackendPlanSkeletonInput {
    pub target: NativeTarget,
    pub host_abi: Arc<HostAbiPlan>,
    pub host_calls: HostCallPlan,
    pub control_flow: Arc<ControlFlowPlan>,
    pub runtime_flow: Arc<RuntimeFlowPlan>,
    pub state_dispatch: StateDispatchPlan,
    pub state_guards: StateGuardPlan,
    pub layouts: LayoutPlan,
    pub entry_key: StateKey,
    pub phase_timings: Arena<BackendPlanPhaseTiming>,
}

pub(super) fn build_backend_plan_skeleton(input: BackendPlanSkeletonInput) -> BackendPlan {
    BackendPlan {
        target: input.target,
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
        machine_instructions: MachineInstructionPlan::default(),
        encoded_machine: EncodedMachinePlan::default(),
        object: ObjectPlan {
            target: input.target,
            sections: omega_core::arena::Arena::new(),
            symbols: omega_core::arena::Arena::new(),
            entry_symbol: Handle::<SymbolPlan>::invalid(),
        },
        relocations: RelocationPlan {
            target: input.target,
            records: omega_core::arena::Arena::new(),
        },
        entry_key: input.entry_key,
        phase_timings: input.phase_timings,
    }
}
