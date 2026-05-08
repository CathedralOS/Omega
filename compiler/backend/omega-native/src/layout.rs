mod builder;
mod packing;
mod sizing;

use crate::layout::builder::LayoutBuilder;
use crate::target::NativeTarget;
use omega_core::diagnostics::Diagnostic;
pub use omega_layout::{DataLayout, DataShape, FieldLayout, LayoutPlan, MachineLayout, TypeLayout};
use omega_typed_program::Program;

pub fn build_layout_plan(
    program: &Program,
    target: NativeTarget,
) -> Result<LayoutPlan, Diagnostic> {
    let mut builder = LayoutBuilder::new(program, target);

    for data_definition in &program.data_definitions {
        builder.layout_data_definition(&data_definition.name)?;
    }

    for machine in &program.machines {
        builder.layout_machine(&machine.name)?;
    }

    Ok(builder.finish())
}
