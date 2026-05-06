use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::native::control_flow::{ControlFlowPlan, build_control_flow_plan};
use crate::native::layout::{LayoutPlan, build_layout_plan};
use crate::native::object::{ObjectPlan, build_object_plan};
use crate::native::target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePlan {
    pub target: NativeTarget,
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
        control_flow: build_control_flow_plan(program)?,
        layouts: build_layout_plan(program, target)?,
        object: ObjectPlan {
            target,
            sections: Vec::new(),
            symbols: Vec::new(),
            entry_symbol: String::new(),
        },
        entry_machine: "main".to_owned(),
        entry_state: "entry".to_owned(),
    };
    native_plan.object = build_object_plan(&native_plan)?;

    Ok(native_plan)
}
