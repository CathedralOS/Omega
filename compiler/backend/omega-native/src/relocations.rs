use crate::plan::NativePlan;
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_object::RelocationPlan;
use omega_target_program::FunctionInstructionPlan;

mod data_addresses;
mod instruction_records;
mod lookups;
mod offsets;

use instruction_records::collect_instruction_relocations;
use lookups::selected_instruction_text_offset;

pub fn build_relocation_plan(native_plan: &NativePlan) -> Result<RelocationPlan, Diagnostic> {
    let mut relocation_plan = RelocationPlan {
        target: native_plan.target,
        records: Arena::new(),
    };

    for (_, function) in native_plan.instructions.functions.iter() {
        collect_function_relocations(native_plan, function, &mut relocation_plan)?;
    }

    Ok(relocation_plan)
}

fn collect_function_relocations(
    native_plan: &NativePlan,
    function: &FunctionInstructionPlan,
    relocation_plan: &mut RelocationPlan,
) -> Result<(), Diagnostic> {
    let Some(instructions) = native_plan
        .instructions
        .instructions
        .span(function.instructions)
    else {
        return Ok(());
    };

    for (offset, instruction) in instructions.iter().enumerate() {
        let selected_instruction_index = function
            .instructions
            .start()
            .arena_index()
            .checked_add(u32::try_from(offset).expect("instruction offset overflow"))
            .expect("instruction index overflow");

        let selected_text_offset =
            selected_instruction_text_offset(native_plan, function, selected_instruction_index)?;

        collect_instruction_relocations(
            native_plan,
            function,
            selected_instruction_index,
            selected_text_offset,
            instruction,
            relocation_plan,
        );
    }

    Ok(())
}
