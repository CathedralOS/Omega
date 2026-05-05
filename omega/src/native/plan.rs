use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::native::control_flow::{ControlFlowPlan, build_control_flow_plan};
use crate::native::layout::{LayoutPlan, build_layout_plan};
use crate::native::target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePlan {
    pub target: NativeTarget,
    pub control_flow: ControlFlowPlan,
    pub layouts: LayoutPlan,
    pub entry_machine: String,
    pub entry_state: String,
}

pub fn build_native_plan(
    program: &Program,
    target: NativeTarget,
) -> Result<NativePlan, Diagnostic> {
    Ok(NativePlan {
        target,
        control_flow: build_control_flow_plan(program)?,
        layouts: build_layout_plan(program, target)?,
        entry_machine: "main".to_owned(),
        entry_state: "Main".to_owned(),
    })
}
