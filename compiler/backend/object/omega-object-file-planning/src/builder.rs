use crate::input::ObjectPlanningInput;
use crate::sections::insert_object_sections;
use crate::symbols::{insert_object_symbols, object_symbol_capacity};
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_layout::MachineLayout;
use omega_machine_bytes::EncodedMachineFunction;
use omega_object_file::ObjectPlan;

pub fn build_object_plan(input: ObjectPlanningInput<'_>) -> Result<ObjectPlan, Diagnostic> {
    let main_layout = entry_machine_layout(&input)?;
    let entry_function = entry_function(&input)?;
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

fn entry_machine_layout<'plan>(
    input: &ObjectPlanningInput<'plan>,
) -> Result<&'plan MachineLayout, Diagnostic> {
    input
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, layout)| layout.symbol == input.entry_machine_symbol)
        .map(|(_, layout)| layout)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "missing native layout for entry machine `{}`",
                input.entry_machine_name
            ))
        })
}

fn entry_function<'plan>(
    input: &ObjectPlanningInput<'plan>,
) -> Result<&'plan EncodedMachineFunction, Diagnostic> {
    input
        .encoded_machine
        .functions
        .iter()
        .find(|(_, function)| function.source_key == input.entry_state_key)
        .map(|(_, function)| function)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "missing encoded entry function for state key {:?}",
                input.entry_state_key
            ))
        })
}
