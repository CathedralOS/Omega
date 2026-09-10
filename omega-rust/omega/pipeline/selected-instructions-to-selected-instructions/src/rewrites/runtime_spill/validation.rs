use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBoundarySettlementPayload, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedLocalStorageSlot, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{RuntimeSpillError, RuntimeSpillReceipt, ValidatedRuntimeSpill, admission};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed instruction stream, checking every private
/// access and rewritten operand against its original use. Stripping these exact
/// additions must restore the entire admitted source, including calls and fuel.
pub fn validate_runtime_spill(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
    let admitted = admission::admit(source, function_index, register, environment, budget)?;
    let function = proposed
        .functions
        .get(function_index)
        .ok_or(RuntimeSpillError::ReplayMismatch)?;
    let mut restored = proposed.clone();
    let restored_function = &mut restored.functions[function_index];
    if restored_function.boundary_settlements.len() != admitted.function.boundary_settlements.len()
    {
        return Err(RuntimeSpillError::ReplayMismatch);
    }
    let mut values = function
        .virtual_registers
        .iter()
        .skip(admitted.function.virtual_registers.len());
    let mut next_instruction = admitted.first_instruction;
    let mut next_register = admitted.first_register;
    for (block_index, source_block) in admitted.function.blocks.iter().enumerate() {
        if !admitted.use_blocks.contains(&block_index)
            && !admitted
                .definitions
                .iter()
                .any(|definition| definition.block_index == block_index)
        {
            continue;
        }
        let block = function
            .blocks
            .get(block_index)
            .ok_or(RuntimeSpillError::ReplayMismatch)?;
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
                let address_instruction =
                    SelectedInstructionId(admission::fresh(&mut next_instruction)?);
                let load_instruction =
                    SelectedInstructionId(admission::fresh(&mut next_instruction)?);
                let address_register = VirtualRegisterId(admission::fresh(&mut next_register)?);
                let reload_register = VirtualRegisterId(admission::fresh(&mut next_register)?);
                let address = VirtualRegister {
                    id: address_register,
                    scalar_type: admitted.address_scalar_type,
                    class: admitted.victim.class,
                    origin: VirtualRegisterOrigin::SpillAddress {
                        instruction: address_instruction,
                        register,
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                };
                let reload = VirtualRegister {
                    id: reload_register,
                    scalar_type: admitted.victim.scalar_type,
                    class: admitted.victim.class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: load_instruction,
                        source_value: admitted.source_value,
                    },
                    definition_site: admitted.victim.definition_site,
                    entry_fixed_view: None,
                };
                if values.next() != Some(&address)
                    || values.next() != Some(&reload)
                    || stream.next()
                        != Some(&admission::instruction(
                            address_instruction,
                            SelectedInstructionKind::FrameAddress {
                                slot: admission::frame(admitted.slot),
                                byte_offset: 0,
                            },
                            admitted.address,
                            &[address_register],
                        ))
                    || stream.next()
                        != Some(&admission::instruction(
                            load_instruction,
                            SelectedInstructionKind::Load64 { byte_offset: 0 },
                            admitted.load,
                            &[address_register, reload_register],
                        ))
                {
                    return Err(RuntimeSpillError::ReplayMismatch);
                }
                consumed = consumed
                    .checked_add(2)
                    .ok_or(RuntimeSpillError::IdentityOverflow)?;
                operand.virtual_register = reload_register;
            }
            instruction_positions.push(consumed);
            if stream.next() != Some(&restored) {
                return Err(RuntimeSpillError::ReplayMismatch);
            }
            consumed = consumed
                .checked_add(1)
                .ok_or(RuntimeSpillError::IdentityOverflow)?;
            for definition in admitted.definitions.iter().filter(|definition| {
                definition.block_index == block_index && original.id == definition.instruction
            }) {
                let store = admission::instruction(
                    SelectedInstructionId(admission::fresh(&mut next_instruction)?),
                    SelectedInstructionKind::Store64 {
                        slot: admission::frame(admitted.slot),
                        byte_offset: 0,
                    },
                    admitted.store,
                    &[definition.register],
                );
                if stream.next() != Some(&store) {
                    return Err(RuntimeSpillError::ReplayMismatch);
                }
                consumed = consumed
                    .checked_add(1)
                    .ok_or(RuntimeSpillError::IdentityOverflow)?;
            }
        }
        boundaries.push(consumed);
        instruction_positions.push(consumed);
        if stream.next().is_some() {
            return Err(RuntimeSpillError::ReplayMismatch);
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
                .ok_or(RuntimeSpillError::SourceMismatch)?;
            if actual.block != source_block.id || actual.instruction_index != *expected {
                return Err(RuntimeSpillError::ReplayMismatch);
            }
            actual.instruction_index = original.instruction_index;
        }
        restored_function.blocks[block_index]
            .instructions
            .clone_from(&source_block.instructions);
    }
    if values.next().is_some()
        || restored_function.local_storage_slots.pop()
            != Some(SelectedLocalStorageSlot {
                id: admitted.slot,
                byte_size: 8,
                alignment: 8,
            })
    {
        return Err(RuntimeSpillError::ReplayMismatch);
    }
    restored_function
        .virtual_registers
        .truncate(admitted.function.virtual_registers.len());
    if restored != *source.selected_plan() {
        return Err(RuntimeSpillError::ReplayMismatch);
    }
    Ok(ValidatedRuntimeSpill {
        receipt: RuntimeSpillReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
