use crate::RelocationPlanningInput;
use crate::instruction_records::collect_instruction_relocations;
use crate::lookups::SelectedInstructionTextLayouts;
use omega_core::diagnostics::Diagnostic;
use omega_object_file::{RelocationPlan, object_symbol_handle_by_name};
use omega_target_operations::FunctionInstructionPlan;

pub fn build_relocation_plan(
    input: RelocationPlanningInput<'_>,
) -> Result<RelocationPlan, Diagnostic> {
    let mut relocation_plan = RelocationPlan::with_record_capacity(
        input.target,
        input.instructions.code.instructions.len(),
    );
    let selected_instruction_text_layouts = SelectedInstructionTextLayouts::collect(input);

    for (_, function) in input.instructions.code.functions.iter() {
        collect_function_relocations(
            input,
            function,
            &selected_instruction_text_layouts,
            &mut relocation_plan,
        )?;
    }

    Ok(relocation_plan)
}

fn collect_function_relocations(
    input: RelocationPlanningInput<'_>,
    function: &FunctionInstructionPlan,
    selected_instruction_text_layouts: &SelectedInstructionTextLayouts,
    relocation_plan: &mut RelocationPlan,
) -> Result<(), Diagnostic> {
    let Some(instructions) = input
        .instructions
        .code
        .instructions
        .span(function.instructions)
    else {
        return Ok(());
    };
    let function_symbol_handle =
        object_symbol_handle_by_name(&input.object, function.symbol.as_ref());

    for (offset, instruction) in instructions.iter().enumerate() {
        let selected_instruction_index = function
            .instructions
            .start()
            .arena_index()
            .checked_add(u32::try_from(offset).expect("instruction offset overflow"))
            .expect("instruction index overflow");

        let selected_text_offset =
            selected_instruction_text_layouts.offset(function, selected_instruction_index)?;
        let selected_text_width =
            selected_instruction_text_layouts.width(selected_instruction_index);

        collect_instruction_relocations(
            input,
            function_symbol_handle,
            selected_instruction_index,
            selected_text_offset,
            selected_text_width,
            instruction,
            relocation_plan,
        );
    }

    Ok(())
}
