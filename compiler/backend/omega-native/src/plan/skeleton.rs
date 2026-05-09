use super::{NativePlan, NativePlanPhaseTiming};
use crate::abi::HostAbiPlan;
use crate::alias_flow::AliasFlowPlan;
use crate::control_flow::{ControlFlowPlan, StateKey};
use crate::data::NativeDataPlan;
use crate::host_calls::HostCallPlan;
use crate::instructions::InstructionPlan;
use crate::machine_code::MachineCodePlan;
use crate::object::ObjectPlan;
use crate::relocations::RelocationPlan;
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
use omega_layout::LayoutPlan;
use omega_target::NativeTarget;

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
        object: ObjectPlan {
            target: input.target,
            sections: omega_core::arena::Arena::new(),
            symbols: omega_core::arena::Arena::new(),
            entry_symbol: String::new(),
        },
        relocations: RelocationPlan {
            target: input.target,
            records: omega_core::arena::Arena::new(),
        },
        entry_key: input.entry_key,
        phase_timings: input.phase_timings,
    }
}
