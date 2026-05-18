use omega_calling_conventions::HostAbiPlan;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_core::allocations::AllocationDelta;
use omega_core::arena::Arena;
use omega_layout::LayoutPlan;
use omega_machine_bytes::EncodedMachinePlan;
use omega_machine_program::MachineProgram;
use omega_object::{ObjectPlan, RelocationPlan};
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
use omega_target_operations::InstructionPlan;
use omega_target_operations::TargetDataPlan;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendPlan {
    pub target: NativeTarget,
    pub host_abi: Arc<HostAbiPlan>,
    pub host_calls: Arc<HostCallPlan>,
    pub state_calls: Arc<StateCallPlan>,
    pub alias_flow: AliasFlowPlan,
    pub state_storage: Arc<StateStoragePlan>,
    pub state_values: StateValuePlan,
    pub data: TargetDataPlan,
    pub instructions: InstructionPlan,
    pub control_flow: Arc<ControlFlowPlan>,
    pub runtime_flow: Arc<RuntimeFlowPlan>,
    pub state_dispatch: Arc<StateDispatchPlan>,
    pub state_guards: Arc<StateGuardPlan>,
    pub runtime_bodies: Arc<RuntimeDispatchBodyPlan>,
    pub runtime_branching_calls: RuntimeBranchingCallPlan,
    pub runtime_dispatch_loop: RuntimeDispatchLoopPlan,
    pub runtime_storage: RuntimeStoragePlan,
    pub runtime_text: RuntimeTextPlan,
    pub layouts: Arc<LayoutPlan>,
    pub machine_program: MachineProgram,
    pub encoded_machine: EncodedMachinePlan,
    pub object: ObjectPlan,
    pub relocations: RelocationPlan,
    pub entry_key: StateKey,
    pub phase_timings: Arena<BackendPlanPhaseTiming>,
}

impl BackendPlan {
    pub fn entry_machine_name(&self) -> &str {
        self.control_flow
            .machine_by_symbol(self.entry_key.machine)
            .map(|machine| machine.name.as_str())
            .unwrap_or("")
    }

    pub fn entry_state_name(&self) -> &str {
        self.control_flow
            .state_by_key(self.entry_key)
            .map(|state| state.name.as_str())
            .unwrap_or("")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackendPlanPhaseTiming {
    pub phase: &'static str,
    pub microseconds: u128,
    pub allocations: AllocationDelta,
}
