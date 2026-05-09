use crate::alias_flow::AliasFlowPlan;
use crate::data::NativeDataPlan;
use crate::instructions::InstructionPlan;
use crate::machine_code::MachineCodePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyPlan;
use crate::runtime_dispatch::branching::RuntimeBranchingCallPlan;
use crate::runtime_dispatch::loop_plan::RuntimeDispatchLoopPlan;
use crate::runtime_flow::RuntimeFlowPlan;
use crate::runtime_storage::RuntimeStoragePlan;
use crate::runtime_text::RuntimeTextPlan;
use crate::state_calls::StateCallPlan;
use crate::state_dispatch::StateDispatchPlan;
use crate::state_guards::StateGuardPlan;
use crate::state_storage::StateStoragePlan;
use crate::state_values::StateValuePlan;
use omega_calling_conventions::HostAbiPlan;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_core::allocations::AllocationDelta;
use omega_layout::LayoutPlan;
use omega_object::{ObjectPlan, RelocationPlan};
use omega_platform_interface::HostCallPlan;
use omega_target::NativeTarget;

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
