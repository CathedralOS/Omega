use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{SelectedBoundarySettlementPayload, VirtualRegisterId};

use super::{RuntimeRematerializationError, ValidatedRuntimeRematerialization, admission};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::runtime_spill::control_mut;

/// Regenerate one immediate-materialized runtime value before each flexible
/// use. The independently checked output is the only admitted result.
pub fn rematerialize_selected_runtime_value(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedRuntimeRematerialization, RuntimeRematerializationError> {
    let admitted = admission::admit(source, function_index, register, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    transformed
        .functions
        .mutate_preserving_register_prefix(
            function_index,
            |_local_storage_slots,
             virtual_registers,
             boundary_settlements,
             blocks|
             -> Result<(), RuntimeRematerializationError> {
                let mut next_instruction = admitted.first_instruction;
                let mut next_register = admitted.first_register;
                for (block_index, block) in admitted.function.blocks.iter().enumerate() {
                    if !admitted.use_blocks.contains(&block_index) {
                        continue;
                    }
                    let has_settlements = boundary_settlements
                        .iter()
                        .any(|settlement| settlement.block == block.id);
                    let mut instructions = Vec::with_capacity(block.instructions.len() + 8);
                    let mut boundaries = if has_settlements {
                        Vec::with_capacity(block.instructions.len() + 1)
                    } else {
                        Vec::new()
                    };
                    let mut instruction_positions = if has_settlements {
                        Vec::with_capacity(block.instructions.len() + 1)
                    } else {
                        Vec::new()
                    };
                    let active_window = if has_settlements || block.instructions.is_empty() {
                        if block.instructions.is_empty() {
                            None
                        } else {
                            Some((0usize, block.instructions.len() - 1))
                        }
                    } else {
                        admitted
                            .function
                            .blocks
                            .block_register_instruction_span(block_index, register)
                    };
                    let unchanged_prefix =
                        active_window.map_or(block.instructions.len(), |(start, _)| start);
                    if let Some((active_start, active_end)) = active_window {
                        if active_start > 0 {
                            instructions.extend_from_slice(&block.instructions[..active_start]);
                        }
                        for original in &block.instructions[active_start..=active_end] {
                            if has_settlements {
                                boundaries.push(
                                    u32::try_from(instructions.len()).map_err(|_| {
                                        RuntimeRematerializationError::IdentityOverflow
                                    })?,
                                );
                            }
                            let mut rewritten = original.clone();
                            for operand in &mut rewritten.operands {
                                if operand.virtual_register != register
                                    || operand.access != RegisterOperandAccess::Use
                                {
                                    continue;
                                }
                                let materialization = admission::materialization(
                                    &admitted,
                                    &mut next_instruction,
                                    &mut next_register,
                                )?;
                                let regenerated = materialization.register.id;
                                virtual_registers.push(materialization.register);
                                instructions.push(materialization.instruction);
                                operand.virtual_register = regenerated;
                            }
                            if has_settlements {
                                instruction_positions.push(
                                    u32::try_from(instructions.len()).map_err(|_| {
                                        RuntimeRematerializationError::IdentityOverflow
                                    })?,
                                );
                            }
                            instructions.push(rewritten);
                        }
                        if active_end + 1 < block.instructions.len() {
                            instructions
                                .extend_from_slice(&block.instructions[active_end + 1..]);
                        }
                    } else if !block.instructions.is_empty() {
                        instructions.extend_from_slice(&block.instructions);
                    }
                    let pre_terminator_len = instructions.len();
                    // Terminator operand uses regenerate after the last block instruction.
                    // Their materializations occupy the end-of-block gap, so the closing
                    // boundary and instruction positions are pushed only after they are
                    // appended.
                    let mut terminator = block.terminator.clone();
                    for operand in &mut control_mut(&mut terminator).operands {
                        if operand.virtual_register != register
                            || operand.access != RegisterOperandAccess::Use
                        {
                            continue;
                        }
                        let materialization = admission::materialization(
                            &admitted,
                            &mut next_instruction,
                            &mut next_register,
                        )?;
                        let regenerated = materialization.register.id;
                        virtual_registers.push(materialization.register);
                        instructions.push(materialization.instruction);
                        operand.virtual_register = regenerated;
                    }
                    if has_settlements {
                        let final_len = u32::try_from(instructions.len())
                            .map_err(|_| RuntimeRematerializationError::IdentityOverflow)?;
                        boundaries.push(final_len);
                        instruction_positions.push(final_len);
                        for settlement in &mut *boundary_settlements {
                            if settlement.block != block.id {
                                continue;
                            }
                            // Eliminated completions retain their gap; hosted settlements name
                            // the surviving instruction, after any newly inserted
                            // materializations.
                            let positions = if matches!(
                                settlement.settlement,
                                SelectedBoundarySettlementPayload::ClaimCompletion(_)
                            ) {
                                &boundaries
                            } else {
                                &instruction_positions
                            };
                            settlement.instruction_index = *positions
                                .get(settlement.instruction_index as usize)
                                .ok_or(RuntimeRematerializationError::SourceMismatch)?;
                        }
                    }
                    let unchanged_suffix = if instructions.len() == pre_terminator_len {
                        active_window
                            .map_or(block.instructions.len(), |(_, end)| {
                                block.instructions.len() - (end + 1)
                            })
                            .min(block.instructions.len() - unchanged_prefix)
                    } else {
                        0
                    };
                    blocks.replace_block_contents(
                        block_index,
                        instructions,
                        terminator,
                        unchanged_prefix,
                        unchanged_suffix,
                    );
                }
                Ok(())
            },
        )?;
    super::validation::seal_runtime_rematerialization(source, transformed)
}
