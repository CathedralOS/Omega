use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::native::abi::{HostAbiPlan, build_host_abi_plan};
use crate::native::alias_flow::{AliasFlowPlan, build_alias_flow_plan};
use crate::native::control_flow::{ControlFlowPlan, build_control_flow_plan};
use crate::native::data::{NativeDataPlan, build_native_data_plan};
use crate::native::host_calls::{HostCallPlan, build_host_call_plan};
use crate::native::instructions::{InstructionPlan, build_instruction_plan};
use crate::native::layout::{LayoutPlan, build_layout_plan};
use crate::native::machine_code::{MachineCodePlan, build_machine_code_plan};
use crate::native::object::{ObjectPlan, build_object_plan};
use crate::native::relocations::{RelocationPlan, build_relocation_plan};
use crate::native::runtime_dispatch::bodies::{
    RuntimeDispatchBodyPlan, build_runtime_dispatch_body_plan,
};
use crate::native::runtime_dispatch::branching::{
    RuntimeBranchingCallPlan, build_runtime_branching_call_plan,
};
use crate::native::runtime_flow::{RuntimeFlowPlan, build_runtime_flow_plan};
use crate::native::runtime_storage::{RuntimeStoragePlan, build_runtime_storage_plan};
use crate::native::runtime_text::{RuntimeTextPlan, build_runtime_text_plan};
use crate::native::state_calls::{StateCallPlan, build_state_call_plan};
use crate::native::state_dispatch::{StateDispatchPlan, build_state_dispatch_plan};
use crate::native::state_guards::{StateGuardPlan, build_state_guard_plan};
use crate::native::state_storage::{StateStoragePlan, build_state_storage_plan};
use crate::native::state_values::{StateValuePlan, build_state_value_plan};
use crate::native::target::NativeTarget;

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
    pub runtime_storage: RuntimeStoragePlan,
    pub runtime_text: RuntimeTextPlan,
    pub layouts: LayoutPlan,
    pub machine_code: MachineCodePlan,
    pub object: ObjectPlan,
    pub relocations: RelocationPlan,
    pub entry_machine: String,
    pub entry_state: String,
}

pub fn build_native_plan(
    program: &Program,
    target: NativeTarget,
) -> Result<NativePlan, Diagnostic> {
    let entry_machine = "main".to_owned();
    let entry_state = "entry".to_owned();
    let control_flow = build_control_flow_plan(program)?;
    let runtime_flow = build_runtime_flow_plan(&control_flow, &entry_machine, &entry_state)?;
    let state_dispatch = build_state_dispatch_plan(&runtime_flow);
    let layouts = build_layout_plan(program, target)?;
    let state_guards = build_state_guard_plan(&state_dispatch, &layouts);

    let mut native_plan = NativePlan {
        target,
        host_abi: build_host_abi_plan(target),
        host_calls: HostCallPlan::default(),
        state_calls: StateCallPlan::default(),
        alias_flow: AliasFlowPlan::default(),
        state_storage: StateStoragePlan::default(),
        state_values: StateValuePlan::default(),
        data: NativeDataPlan::default(),
        instructions: InstructionPlan {
            target,
            functions: omega_core::arena::Arena::new(),
            instructions: omega_core::arena::Arena::new(),
            operands: omega_core::arena::Arena::new(),
        },
        control_flow,
        runtime_flow,
        state_dispatch,
        state_guards,
        runtime_bodies: RuntimeDispatchBodyPlan::default(),
        runtime_branching_calls: RuntimeBranchingCallPlan::default(),
        runtime_storage: RuntimeStoragePlan::default(),
        runtime_text: RuntimeTextPlan::default(),
        layouts,
        machine_code: MachineCodePlan::default(),
        object: ObjectPlan {
            target,
            sections: omega_core::arena::Arena::new(),
            symbols: omega_core::arena::Arena::new(),
            entry_symbol: String::new(),
        },
        relocations: RelocationPlan {
            target,
            records: omega_core::arena::Arena::new(),
        },
        entry_machine,
        entry_state,
    };
    native_plan.host_calls = build_host_call_plan(program, target, &native_plan.host_abi)?;
    native_plan.state_calls = build_state_call_plan(&native_plan);
    native_plan.alias_flow = build_alias_flow_plan(&native_plan);
    native_plan.state_storage = build_state_storage_plan(program, &native_plan);
    native_plan.state_values = build_state_value_plan(program, &native_plan);
    native_plan.runtime_bodies = build_runtime_dispatch_body_plan(&native_plan);
    native_plan.runtime_branching_calls = build_runtime_branching_call_plan(&native_plan);
    native_plan.runtime_storage = build_runtime_storage_plan(&native_plan);
    native_plan.runtime_text = build_runtime_text_plan(&native_plan);
    native_plan.data = build_native_data_plan(&native_plan.host_calls);
    native_plan.object = build_object_plan(&native_plan)?;
    native_plan.instructions = build_instruction_plan(&native_plan);
    native_plan.machine_code = build_machine_code_plan(&native_plan)?;
    native_plan.object = build_object_plan(&native_plan)?;
    native_plan.relocations = build_relocation_plan(&native_plan)?;

    Ok(native_plan)
}
