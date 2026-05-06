use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::native::abi::{HostAbiPlan, build_host_abi_plan};
use crate::native::control_flow::{ControlFlowPlan, build_control_flow_plan};
use crate::native::host_calls::{HostCallPlan, build_host_call_plan};
use crate::native::layout::{LayoutPlan, build_layout_plan};
use crate::native::object::{ObjectPlan, build_object_plan};
use crate::native::target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePlan {
    pub target: NativeTarget,
    pub host_abi: HostAbiPlan,
    pub host_calls: HostCallPlan,
    pub control_flow: ControlFlowPlan,
    pub layouts: LayoutPlan,
    pub object: ObjectPlan,
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
        host_calls: build_host_call_plan(program, target),
        control_flow: build_control_flow_plan(program)?,
        layouts: build_layout_plan(program, target)?,
        object: ObjectPlan {
            target,
            sections: omega_core::arena::Arena::new(),
            symbols: omega_core::arena::Arena::new(),
            entry_symbol: String::new(),
        },
        entry_machine: "main".to_owned(),
        entry_state: "entry".to_owned(),
    };
    native_plan.object = build_object_plan(&native_plan)?;

    Ok(native_plan)
}
