//! Ordinary function and block construction in source order.

use omega_regalloc::ValidatedRegisterHomes;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{SelectedBlock, SelectedInstruction, SelectedInstructionPlan};

use crate::{
    BlockMachineEffects, PostAllocationMachineBlock, PostAllocationMachineError,
    PostAllocationMachineFunction, ValidatedPreAllocationMachineEffects,
};

use super::instruction;

pub(super) fn build_functions(
    selected: &SelectedInstructionPlan,
    effects: &ValidatedPreAllocationMachineEffects,
    homes: &ValidatedRegisterHomes,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Vec<PostAllocationMachineFunction>, PostAllocationMachineError> {
    selected
        .functions
        .iter()
        .enumerate()
        .map(|(function_index, function)| {
            let effect_function = effects
                .plan()
                .functions
                .get(function_index)
                .filter(|effects| effects.machine == function.machine)
                .ok_or(PostAllocationMachineError::FunctionMismatch {
                    function: function_index,
                })?;
            let home_function = homes
                .plan()
                .functions
                .iter()
                .find(|homes| homes.machine == function.machine)
                .ok_or(PostAllocationMachineError::FunctionMismatch {
                    function: function_index,
                })?;
            if effect_function.blocks.len() != function.blocks.len() {
                return Err(PostAllocationMachineError::FunctionMismatch {
                    function: function_index,
                });
            }
            let blocks = function
                .blocks
                .iter()
                .enumerate()
                .map(|(block_index, block)| {
                    build_block(
                        function_index,
                        block_index,
                        block,
                        &effect_function.blocks[block_index],
                        home_function,
                        physical,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PostAllocationMachineFunction {
                machine: function.machine,
                blocks,
            })
        })
        .collect()
}

fn build_block(
    function_index: usize,
    block_index: usize,
    selected: &SelectedBlock,
    effects: &BlockMachineEffects,
    homes: &omega_regalloc::FunctionRegisterHomes,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<PostAllocationMachineBlock, PostAllocationMachineError> {
    if effects.block != selected.id {
        return Err(PostAllocationMachineError::BlockMismatch {
            function: function_index,
            block: block_index,
        });
    }
    let selected_instructions = selected_instructions(selected).collect::<Vec<_>>();
    if effects.instructions.len() != selected_instructions.len() {
        return Err(PostAllocationMachineError::BlockMismatch {
            function: function_index,
            block: block_index,
        });
    }
    let instructions = selected_instructions
        .into_iter()
        .zip(&effects.instructions)
        .map(|(selected, effects)| {
            instruction::build(function_index, selected, effects, homes, physical)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PostAllocationMachineBlock {
        block: selected.id,
        instructions,
    })
}

fn selected_instructions(block: &SelectedBlock) -> impl Iterator<Item = &SelectedInstruction> {
    let terminator = match &block.terminator {
        omega_selected_instructions::SelectedTerminator::ConditionalBranch {
            instruction, ..
        }
        | omega_selected_instructions::SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            ..
        }
        | omega_selected_instructions::SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            ..
        }
        | omega_selected_instructions::SelectedTerminator::Return { instruction, .. } => {
            instruction
        }
    };
    block.instructions.iter().chain(std::iter::once(terminator))
}
