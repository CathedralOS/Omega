use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBoundarySettlementPayload, SelectedInstructionPlan, VirtualRegisterId,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    RuntimeRematerializationError, RuntimeRematerializationReceipt,
    ValidatedRuntimeRematerialization, admission,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::runtime_spill::control_mut;

/// Independently consume the proposed instruction stream, checking every fresh
/// materialization and rewritten operand against its original use. Stripping
/// these exact additions must restore the entire admitted source, including
/// calls and fuel.
pub fn validate_runtime_rematerialization(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRuntimeRematerialization, RuntimeRematerializationError> {
    let admitted = admission::admit(source, function_index, register, environment, budget)?;
    let function = proposed
        .functions
        .get(function_index)
        .ok_or(RuntimeRematerializationError::ReplayMismatch)?;
    let mut restored = proposed.clone();
    let restored_function = &mut restored.functions[function_index];
    if restored_function.boundary_settlements.len() != admitted.function.boundary_settlements.len()
    {
        return Err(RuntimeRematerializationError::ReplayMismatch);
    }
    let mut values = function
        .virtual_registers
        .iter()
        .skip(admitted.function.virtual_registers.len());
    let mut next_instruction = admitted.first_instruction;
    let mut next_register = admitted.first_register;
    for (block_index, source_block) in admitted.function.blocks.iter().enumerate() {
        if !admitted.use_blocks.contains(&block_index) {
            continue;
        }
        let block = function
            .blocks
            .get(block_index)
            .ok_or(RuntimeRematerializationError::ReplayMismatch)?;
        let mut stream = block.instructions.iter();
        let mut boundaries = Vec::new();
        let mut instruction_positions = Vec::new();
        let mut consumed = 0u32;
        for original in &source_block.instructions {
            boundaries.push(consumed);
            let mut restored = original.clone();
            for operand in &mut restored.operands {
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
                if values.next() != Some(&materialization.register)
                    || stream.next() != Some(&materialization.instruction)
                {
                    return Err(RuntimeRematerializationError::ReplayMismatch);
                }
                consumed = consumed
                    .checked_add(1)
                    .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
                operand.virtual_register = materialization.register.id;
            }
            instruction_positions.push(consumed);
            if stream.next() != Some(&restored) {
                return Err(RuntimeRematerializationError::ReplayMismatch);
            }
            consumed = consumed
                .checked_add(1)
                .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
        }
        // The rewrite appends terminator-operand materializations after the
        // last body instruction; replay consumes them in operand order, then
        // requires the proposed terminator to equal the source with exactly
        // those operands redirected to their fresh registers.
        let mut expected_terminator = source_block.terminator.clone();
        for operand in &mut control_mut(&mut expected_terminator).operands {
            if operand.virtual_register != register || operand.access != RegisterOperandAccess::Use
            {
                continue;
            }
            let materialization =
                admission::materialization(&admitted, &mut next_instruction, &mut next_register)?;
            if values.next() != Some(&materialization.register)
                || stream.next() != Some(&materialization.instruction)
            {
                return Err(RuntimeRematerializationError::ReplayMismatch);
            }
            consumed = consumed
                .checked_add(1)
                .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
            operand.virtual_register = materialization.register.id;
        }
        if block.terminator != expected_terminator {
            return Err(RuntimeRematerializationError::ReplayMismatch);
        }
        restored_function.blocks[block_index]
            .terminator
            .clone_from(&source_block.terminator);
        boundaries.push(consumed);
        instruction_positions.push(consumed);
        if stream.next().is_some() {
            return Err(RuntimeRematerializationError::ReplayMismatch);
        }
        for (actual, original) in restored_function
            .boundary_settlements
            .iter_mut()
            .zip(&admitted.function.boundary_settlements)
        {
            if original.block != source_block.id {
                continue;
            }
            let positions = if matches!(
                original.settlement,
                SelectedBoundarySettlementPayload::ClaimCompletion(_)
            ) {
                &boundaries
            } else {
                &instruction_positions
            };
            let expected = positions
                .get(original.instruction_index as usize)
                .ok_or(RuntimeRematerializationError::SourceMismatch)?;
            if actual.block != source_block.id || actual.instruction_index != *expected {
                return Err(RuntimeRematerializationError::ReplayMismatch);
            }
            actual.instruction_index = original.instruction_index;
        }
        restored_function.blocks[block_index]
            .instructions
            .clone_from(&source_block.instructions);
    }
    if values.next().is_some() {
        return Err(RuntimeRematerializationError::ReplayMismatch);
    }
    restored_function
        .virtual_registers
        .truncate(admitted.function.virtual_registers.len());
    if restored != *source.selected_plan() {
        return Err(RuntimeRematerializationError::ReplayMismatch);
    }
    Ok(ValidatedRuntimeRematerialization {
        receipt: RuntimeRematerializationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
