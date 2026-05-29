use crate::input::ObjectPlanningInput;
use crate::sections::insert_object_sections;
use crate::symbols::{insert_object_symbols, object_symbol_capacity};
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_object_file::ObjectPlan;

pub fn build_object_plan(input: ObjectPlanningInput<'_>) -> Result<ObjectPlan, Diagnostic> {
    let main_layout = crate::entry::entry_machine_layout(&input)?;
    let entry_function = crate::entry::entry_function(&input)?;
    let mut object_plan = ObjectPlan {
        target: input.target,
        sections: Arena::with_capacity(3),
        symbols: Arena::with_capacity(object_symbol_capacity(&input)),
        entry_symbol: omega_core::arena::Handle::invalid(),
    };
    let section_layout = insert_object_sections(&input, main_layout, &mut object_plan);
    insert_object_symbols(
        &input,
        main_layout,
        entry_function,
        section_layout.runtime_frame_offset,
        &mut object_plan,
    );

    Ok(object_plan)
}
