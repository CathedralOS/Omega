use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::native::abi::{HostAbiPlan, build_host_abi_plan};
use crate::native::control_flow::{ControlFlowPlan, build_control_flow_plan};
use crate::native::data::{NativeDataPlan, build_native_data_plan};
use crate::native::host_calls::{HostCallPlan, build_host_call_plan};
use crate::native::instructions::{InstructionPlan, build_instruction_plan};
use crate::native::layout::{LayoutPlan, build_layout_plan};
use crate::native::machine_code::{MachineCodePlan, build_machine_code_plan};
use crate::native::object::{ObjectPlan, build_object_plan};
use crate::native::relocations::{RelocationPlan, build_relocation_plan};
use crate::native::target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePlan {
    pub target: NativeTarget,
    pub host_abi: HostAbiPlan,
    pub host_calls: HostCallPlan,
    pub data: NativeDataPlan,
    pub instructions: InstructionPlan,
    pub control_flow: ControlFlowPlan,
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
    let mut native_plan = NativePlan {
        target,
        host_abi: build_host_abi_plan(target),
        host_calls: HostCallPlan::default(),
        data: NativeDataPlan::default(),
        instructions: InstructionPlan {
            target,
            functions: omega_core::arena::Arena::new(),
            instructions: omega_core::arena::Arena::new(),
            operands: omega_core::arena::Arena::new(),
        },
        control_flow: build_control_flow_plan(program)?,
        layouts: build_layout_plan(program, target)?,
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
        entry_machine: "main".to_owned(),
        entry_state: "entry".to_owned(),
    };
    native_plan.host_calls = build_host_call_plan(program, target)?;
    native_plan.data = build_native_data_plan(&native_plan.host_calls);
    native_plan.object = build_object_plan(&native_plan)?;
    native_plan.instructions = build_instruction_plan(&native_plan);
    native_plan.machine_code = build_machine_code_plan(&native_plan)?;
    native_plan.object = build_object_plan(&native_plan)?;
    native_plan.relocations = build_relocation_plan(&native_plan);

    Ok(native_plan)
}
