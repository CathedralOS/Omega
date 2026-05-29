use crate::RelocationPlanningInput;
use crate::instruction_records::collect_instruction_relocations;
use crate::lookups::selected_instruction_text_offset;
use omega_core::arena::{Arena, Handle};
use omega_core::diagnostics::Diagnostic;
use omega_object_file::{RelocationPlan, object_symbol_handle_by_name};
use omega_target_operations::FunctionInstructionPlan;

pub fn build_relocation_plan(
    input: RelocationPlanningInput<'_>,
) -> Result<RelocationPlan, Diagnostic> {
    let mut relocation_plan = RelocationPlan {
        target: input.target,
        records: Arena::new(),
    };

    for (function_handle, function) in input.instructions.code.functions.iter() {
        collect_function_relocations(input, function_handle, function, &mut relocation_plan)?;
    }

    Ok(relocation_plan)
}

fn collect_function_relocations(
    input: RelocationPlanningInput<'_>,
    function_handle: Handle<FunctionInstructionPlan>,
    function: &FunctionInstructionPlan,
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

        let selected_text_offset = selected_instruction_text_offset(
            input,
            function_handle,
            function,
            selected_instruction_index,
        )?;

        collect_instruction_relocations(
            input,
            function_symbol_handle,
            selected_instruction_index,
            selected_text_offset,
            instruction,
            relocation_plan,
        );
    }

    Ok(())
}
