use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBoundarySettlementPayload, SelectedCasePayloadTransport, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedLocalStorageSlot,
    SelectedValueTransport, VirtualRegisterId,
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
        // Replay reconstructs the same block-local decision independently:
        // when admission proved a surviving view, the first unpinned
        // instruction-operand use emits the pair and every later unpinned use
        // names that still-open reload register with no further pair. A pinned
        // operand or an unadmitted block consumes a fresh pair at every use.
        let shared = admitted.shared_reload[block_index];
        let mut open_reload: Option<VirtualRegisterId> = None;
        for original in &source_block.instructions {
            boundaries.push(consumed);
            let mut restored = original.clone();
            for operand in &mut restored.operands {
                if operand.virtual_register != register
                    || operand.access != RegisterOperandAccess::Use
                {
                    continue;
                }
                let share = shared && operand.fixed_view.is_none();
                let reloaded = match (share, open_reload) {
                    (true, Some(existing)) => existing,
                    _ => {
                        let reload = admission::reload(
                            &admitted,
                            register,
                            &mut next_instruction,
                            &mut next_register,
                        )?;
                        if values.next() != Some(&reload.address_register)
                            || values.next() != Some(&reload.reload_register)
                            || stream.next() != Some(&reload.address)
                            || stream.next() != Some(&reload.load)
                        {
                            return Err(RuntimeSpillError::ReplayMismatch);
                        }
                        consumed = consumed
                            .checked_add(2)
                            .ok_or(RuntimeSpillError::IdentityOverflow)?;
                        if share {
                            open_reload = Some(reload.reload_register.id);
                        }
                        reload.reload_register.id
                    }
                };
                operand.virtual_register = reloaded;
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
        // The rewrite appends terminator-operand reloads after the last body
        // instruction; replay consumes them in operand order, then requires
        // the proposed terminator to equal the source with exactly those
        // operands redirected to their reload registers.
        let mut expected_terminator = source_block.terminator.clone();
        for operand in &mut super::control_mut(&mut expected_terminator).operands {
            if operand.virtual_register != register || operand.access != RegisterOperandAccess::Use
            {
                continue;
            }
            let reload = admission::reload(
                &admitted,
                register,
                &mut next_instruction,
                &mut next_register,
            )?;
            if values.next() != Some(&reload.address_register)
                || values.next() != Some(&reload.reload_register)
                || stream.next() != Some(&reload.address)
                || stream.next() != Some(&reload.load)
            {
                return Err(RuntimeSpillError::ReplayMismatch);
            }
            consumed = consumed
                .checked_add(2)
                .ok_or(RuntimeSpillError::IdentityOverflow)?;
            operand.virtual_register = reload.reload_register.id;
        }
        // Binding-argument reloads follow the terminator-operand pairs in
        // successor, then binding, then case-payload order; the expected
        // terminator carries the moved argument on each matching transport
        // and nothing else.
        for successor in super::control_successors_mut(&mut expected_terminator)
            .into_iter()
            .flatten()
        {
            for binding in &mut successor.bindings {
                if !matches!(
                    binding.transport,
                    SelectedValueTransport::Registers { argument, .. } if argument == register)
                {
                    continue;
                }
                let reload = admission::reload(
                    &admitted,
                    register,
                    &mut next_instruction,
                    &mut next_register,
                )?;
                if values.next() != Some(&reload.address_register)
                    || values.next() != Some(&reload.reload_register)
                    || stream.next() != Some(&reload.address)
                    || stream.next() != Some(&reload.load)
                {
                    return Err(RuntimeSpillError::ReplayMismatch);
                }
                consumed = consumed
                    .checked_add(2)
                    .ok_or(RuntimeSpillError::IdentityOverflow)?;
                let SelectedValueTransport::Registers { argument, .. } = &mut binding.transport
                else {
                    unreachable!()
                };
                *argument = reload.reload_register.id;
            }
            if let Some(case) = &mut successor.structural_case {
                for payload in &mut case.payloads {
                    if !matches!(
                        payload.transport,
                        SelectedCasePayloadTransport::Registers { argument, .. }
                            if argument == register)
                    {
                        continue;
                    }
                    let reload = admission::reload(
                        &admitted,
                        register,
                        &mut next_instruction,
                        &mut next_register,
                    )?;
                    if values.next() != Some(&reload.address_register)
                        || values.next() != Some(&reload.reload_register)
                        || stream.next() != Some(&reload.address)
                        || stream.next() != Some(&reload.load)
                    {
                        return Err(RuntimeSpillError::ReplayMismatch);
                    }
                    consumed = consumed
                        .checked_add(2)
                        .ok_or(RuntimeSpillError::IdentityOverflow)?;
                    let SelectedCasePayloadTransport::Registers { argument, .. } =
                        &mut payload.transport
                    else {
                        unreachable!()
                    };
                    *argument = reload.reload_register.id;
                }
            }
        }
        if block.terminator != expected_terminator {
            return Err(RuntimeSpillError::ReplayMismatch);
        }
        restored_function.blocks[block_index]
            .terminator
            .clone_from(&source_block.terminator);
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
