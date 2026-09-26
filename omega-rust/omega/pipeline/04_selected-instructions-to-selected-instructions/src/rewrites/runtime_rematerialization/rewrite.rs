use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{SelectedBoundarySettlementPayload, VirtualRegisterId};

use super::{
    RuntimeRematerializationError, ValidatedRuntimeRematerialization, admission,
    validate_runtime_rematerialization,
};
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
    let function = &mut transformed.functions[function_index];
    let mut next_instruction = admitted.first_instruction;
    let mut next_register = admitted.first_register;
    for (block_index, block) in admitted.function.blocks.iter().enumerate() {
        if !admitted.use_blocks.contains(&block_index) {
            continue;
        }
        let mut instructions = Vec::new();
        let mut boundaries = Vec::new();
        let mut instruction_positions = Vec::new();
        for original in &block.instructions {
            boundaries.push(
                u32::try_from(instructions.len())
                    .map_err(|_| RuntimeRematerializationError::IdentityOverflow)?,
            );
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
                function.virtual_registers.push(materialization.register);
                instructions.push(materialization.instruction);
                operand.virtual_register = regenerated;
            }
            instruction_positions.push(
                u32::try_from(instructions.len())
                    .map_err(|_| RuntimeRematerializationError::IdentityOverflow)?,
            );
            instructions.push(rewritten);
        }
        // Terminator operand uses regenerate after the last block instruction.
        // Their materializations occupy the end-of-block gap, so the closing
        // boundary and instruction positions are pushed only after they are
        // appended.
        let mut terminator = block.terminator.clone();
        for operand in &mut control_mut(&mut terminator).operands {
            if operand.virtual_register != register || operand.access != RegisterOperandAccess::Use
            {
                continue;
            }
            let materialization =
                admission::materialization(&admitted, &mut next_instruction, &mut next_register)?;
            let regenerated = materialization.register.id;
            function.virtual_registers.push(materialization.register);
            instructions.push(materialization.instruction);
            operand.virtual_register = regenerated;
        }
        boundaries.push(
            u32::try_from(instructions.len())
                .map_err(|_| RuntimeRematerializationError::IdentityOverflow)?,
        );
        instruction_positions.push(
            u32::try_from(instructions.len())
                .map_err(|_| RuntimeRematerializationError::IdentityOverflow)?,
        );
        for settlement in &mut function.boundary_settlements {
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
        function.blocks[block_index].instructions = instructions;
        function.blocks[block_index].terminator = terminator;
    }
    validate_runtime_rematerialization(
        source,
        function_index,
        register,
        environment,
        budget,
        transformed,
    )
}
