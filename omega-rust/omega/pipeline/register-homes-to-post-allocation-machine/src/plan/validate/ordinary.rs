use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::{ValidatedRegisterHomes, ValidatedSelectedAnalysis};

use crate::PostAllocationMachineError;
use physical_instructions::PostAllocationMachinePlan;
use selected_instructions_to_register_homes::ValidatedPreAllocationMachineEffects;

use super::instruction::reconstruct_instruction;

pub(super) fn validate_ordinary_functions<S: ValidatedSelectedAnalysis>(
    selected: &S,
    effects: &ValidatedPreAllocationMachineEffects,
    homes: &ValidatedRegisterHomes,
    physical: &ValidatedPhysicalRegisterModel,
    plan: &PostAllocationMachinePlan,
) -> Result<(), PostAllocationMachineError> {
    if plan.functions.len() != selected.selected_plan().functions.len()
        || plan.functions.len() != effects.plan().functions.len()
        || plan.functions.len() != homes.plan().functions.len()
    {
        return Err(PostAllocationMachineError::FunctionMismatch { function: 0 });
    }
    for (function_index, ((selected_function, effect_function), actual_function)) in selected
        .selected_plan()
        .functions
        .iter()
        .zip(&effects.plan().functions)
        .zip(&plan.functions)
        .enumerate()
    {
        let home_function = homes
            .plan()
            .functions
            .iter()
            .find(|homes| homes.machine == selected_function.machine)
            .ok_or(PostAllocationMachineError::FunctionMismatch {
                function: function_index,
            })?;
        if effect_function.machine != selected_function.machine
            || actual_function.machine != selected_function.machine
            || effect_function.blocks.len() != selected_function.blocks.len()
            || actual_function.blocks.len() != selected_function.blocks.len()
        {
            return Err(PostAllocationMachineError::FunctionMismatch {
                function: function_index,
            });
        }
        for (block_index, ((selected_block, effect_block), actual_block)) in selected_function
            .blocks
            .iter()
            .zip(&effect_function.blocks)
            .zip(&actual_function.blocks)
            .enumerate()
        {
            if effect_block.block != selected_block.id || actual_block.block != selected_block.id {
                return Err(PostAllocationMachineError::BlockMismatch {
                    function: function_index,
                    block: block_index,
                });
            }
            let selected_instructions =
                super::instruction::selected_instructions(selected_block).collect::<Vec<_>>();
            if effect_block.instructions.len() != selected_instructions.len()
                || actual_block.instructions.len() != selected_instructions.len()
            {
                return Err(PostAllocationMachineError::BlockMismatch {
                    function: function_index,
                    block: block_index,
                });
            }
            for ((selected_instruction, effect_instruction), actual_instruction) in
                selected_instructions
                    .into_iter()
                    .zip(&effect_block.instructions)
                    .zip(&actual_block.instructions)
            {
                let expected = reconstruct_instruction(
                    function_index,
                    selected_instruction,
                    effect_instruction,
                    home_function,
                    physical,
                )?;
                if &expected != actual_instruction {
                    return Err(PostAllocationMachineError::InstructionMismatch {
                        function: function_index,
                        instruction: selected_instruction.id.0,
                    });
                }
            }
        }
    }
    Ok(())
}
