use super::{NativePlan, NativePlanPhaseTiming};
use crate::runtime_dispatch::branching::RuntimeBranchingCallPlan;
use crate::runtime_dispatch::loop_plan::RuntimeDispatchLoopPlan;
use omega_calling_conventions::HostAbiPlan;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_layout::LayoutPlan;
use omega_machine_program::{EncodedMachinePlan, MachineCodePlan};
use omega_object::{ObjectPlan, RelocationPlan, entry_symbol_name};
use omega_platform_interface::HostCallPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use omega_runtime_text::RuntimeTextPlan;
use omega_state_calls::{AliasFlowPlan, StateCallPlan};
use omega_state_dispatch::StateDispatchPlan;
use omega_state_graph::RuntimeFlowPlan;
use omega_state_guards::StateGuardPlan;
use omega_state_storage::StateStoragePlan;
use omega_state_values::StateValuePlan;
use omega_target::NativeTarget;
use omega_target_program::InstructionPlan;
use omega_target_program::NativeDataPlan;

pub(super) struct NativePlanSkeletonInput {
    pub target: NativeTarget,
    pub host_abi: HostAbiPlan,
    pub host_calls: HostCallPlan,
    pub control_flow: ControlFlowPlan,
    pub runtime_flow: RuntimeFlowPlan,
    pub state_dispatch: StateDispatchPlan,
    pub state_guards: StateGuardPlan,
    pub layouts: LayoutPlan,
    pub entry_key: StateKey,
    pub phase_timings: Vec<NativePlanPhaseTiming>,
}

pub(super) fn build_native_plan_skeleton(input: NativePlanSkeletonInput) -> NativePlan {
    NativePlan {
        target: input.target,
        host_abi: input.host_abi,
        host_calls: input.host_calls,
        state_calls: StateCallPlan::default(),
        alias_flow: AliasFlowPlan::default(),
        state_storage: StateStoragePlan::default(),
        state_values: StateValuePlan::default(),
        data: NativeDataPlan::default(),
        instructions: InstructionPlan::default(),
        control_flow: input.control_flow,
        runtime_flow: input.runtime_flow,
        state_dispatch: input.state_dispatch,
        state_guards: input.state_guards,
        runtime_bodies: RuntimeDispatchBodyPlan::default(),
        runtime_branching_calls: RuntimeBranchingCallPlan::default(),
        runtime_dispatch_loop: RuntimeDispatchLoopPlan::default(),
        runtime_storage: RuntimeStoragePlan::default(),
        runtime_text: RuntimeTextPlan::default(),
        layouts: input.layouts,
        machine_code: MachineCodePlan::default(),
        encoded_machine: EncodedMachinePlan::default(),
        object: ObjectPlan {
            target: input.target,
            sections: omega_core::arena::Arena::new(),
            symbols: omega_core::arena::Arena::new(),
            entry_symbol: entry_symbol_name(input.target),
        },
        relocations: RelocationPlan {
            target: input.target,
            records: omega_core::arena::Arena::new(),
        },
        entry_key: input.entry_key,
        phase_timings: input.phase_timings,
    }
}
