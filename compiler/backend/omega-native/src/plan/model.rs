use crate::runtime_dispatch::branching::RuntimeBranchingCallPlan;
use crate::runtime_dispatch::loop_plan::RuntimeDispatchLoopPlan;
use crate::state_guards::StateGuardPlan;
use omega_calling_conventions::HostAbiPlan;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_core::allocations::AllocationDelta;
use omega_layout::LayoutPlan;
use omega_machine_program::{EncodedMachinePlan, MachineCodePlan};
use omega_object::{ObjectPlan, RelocationPlan};
use omega_platform_interface::HostCallPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use omega_runtime_text::RuntimeTextPlan;
use omega_state_calls::{AliasFlowPlan, StateCallPlan};
use omega_state_dispatch::StateDispatchPlan;
use omega_state_graph::RuntimeFlowPlan;
use omega_state_storage::StateStoragePlan;
use omega_state_values::StateValuePlan;
use omega_target::NativeTarget;
use omega_target_program::InstructionPlan;
use omega_target_program::NativeDataPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePlan {
    pub target: NativeTarget,
    pub host_abi: HostAbiPlan,
    pub host_calls: HostCallPlan,
    pub state_calls: StateCallPlan,
    pub alias_flow: AliasFlowPlan,
    pub state_storage: StateStoragePlan,
    pub state_values: StateValuePlan,
    pub data: NativeDataPlan,
    pub instructions: InstructionPlan,
    pub control_flow: ControlFlowPlan,
    pub runtime_flow: RuntimeFlowPlan,
    pub state_dispatch: StateDispatchPlan,
    pub state_guards: StateGuardPlan,
    pub runtime_bodies: RuntimeDispatchBodyPlan,
    pub runtime_branching_calls: RuntimeBranchingCallPlan,
    pub runtime_dispatch_loop: RuntimeDispatchLoopPlan,
    pub runtime_storage: RuntimeStoragePlan,
    pub runtime_text: RuntimeTextPlan,
    pub layouts: LayoutPlan,
    pub machine_code: MachineCodePlan,
    pub encoded_machine: EncodedMachinePlan,
    pub object: ObjectPlan,
    pub relocations: RelocationPlan,
    pub entry_key: StateKey,
    pub phase_timings: Vec<NativePlanPhaseTiming>,
}

impl NativePlan {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePlanPhaseTiming {
    pub phase: String,
    pub microseconds: u128,
    pub allocations: AllocationDelta,
}
