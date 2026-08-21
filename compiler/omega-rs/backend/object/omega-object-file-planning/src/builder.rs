use crate::input::ObjectPlanningInput;
use crate::sections::insert_object_sections;
use crate::symbols::{insert_object_symbols, object_symbol_capacity};
use omega_object_file::ObjectPlan;
use psi_diagnostics::Diagnostic;

pub fn build_object_plan(input: ObjectPlanningInput<'_>) -> Result<ObjectPlan, Diagnostic> {
    let main_layout = crate::entry::entry_machine_layout(&input)?;
    let entry_function = crate::entry::entry_function(&input)?;
    let mut object_plan = ObjectPlan::with_capacities(
        input.target,
        3,
        object_symbol_capacity(&input),
        input.encoded_machine.code.functions.len(),
    );
    let section_layout = insert_object_sections(&input, main_layout, &mut object_plan);
    insert_object_symbols(
        &input,
        main_layout,
        entry_function,
        section_layout.runtime_frame_offset,
        &mut object_plan,
    )?;

    Ok(object_plan)
}
