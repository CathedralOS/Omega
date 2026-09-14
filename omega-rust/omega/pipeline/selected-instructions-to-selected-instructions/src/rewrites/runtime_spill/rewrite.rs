use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBoundarySettlementPayload, SelectedInstructionId, SelectedInstructionKind,
    SelectedLocalStorageSlot, SelectedValueTransport, VirtualRegisterId,
};

use super::{RuntimeSpillError, ValidatedRuntimeSpill, admission, validate_runtime_spill};
use crate::ValidatedSelectedAnalysis;

/// Store one nonaddress runtime value after its definition and reload before each
/// flexible use. The independently checked output is the only admitted result.
pub fn spill_selected_runtime_value(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
    let admitted = admission::admit(source, function_index, register, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    function.local_storage_slots.push(SelectedLocalStorageSlot {
        id: admitted.slot,
        byte_size: 8,
        alignment: 8,
    });
    let mut next_instruction = admitted.first_instruction;
    let mut next_register = admitted.first_register;
    for (block_index, block) in admitted.function.blocks.iter().enumerate() {
        if !admitted.use_blocks.contains(&block_index)
            && !admitted
                .definitions
                .iter()
                .any(|definition| definition.block_index == block_index)
        {
            continue;
        }
        let mut instructions = Vec::new();
        let mut boundaries = Vec::new();
        let mut instruction_positions = Vec::new();
        for original in &block.instructions {
            boundaries.push(
                u32::try_from(instructions.len())
                    .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
            );
            let mut rewritten = original.clone();
            for operand in &mut rewritten.operands {
                if operand.virtual_register != register
                    || operand.access != RegisterOperandAccess::Use
                {
                    continue;
                }
                let reload = admission::reload(
                    &admitted,
                    register,
                    &mut next_instruction,
                    &mut next_register,
                )?;
                let reloaded = reload.reload_register.id;
                function.virtual_registers.push(reload.address_register);
                function.virtual_registers.push(reload.reload_register);
                instructions.push(reload.address);
                instructions.push(reload.load);
                operand.virtual_register = reloaded;
            }
            instruction_positions.push(
                u32::try_from(instructions.len())
                    .map_err(|_| RuntimeSpillError::IdentityOverflow)?,
            );
            instructions.push(rewritten);
            for definition in admitted.definitions.iter().filter(|definition| {
                definition.block_index == block_index && original.id == definition.instruction
            }) {
                instructions.push(admission::instruction(
                    SelectedInstructionId(admission::fresh(&mut next_instruction)?),
                    SelectedInstructionKind::Store64 {
                        slot: admission::frame(admitted.slot),
                        byte_offset: 0,
                    },
                    admitted.store,
                    &[definition.register],
                ));
            }
        }
        // Terminator operand uses reload after the last block instruction.
        // Their pairs occupy the end-of-block gap, so the closing boundary and
        // instruction positions are pushed only after they are appended.
        let mut terminator = block.terminator.clone();
        for operand in &mut super::control_mut(&mut terminator).operands {
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
            let reloaded = reload.reload_register.id;
            function.virtual_registers.push(reload.address_register);
            function.virtual_registers.push(reload.reload_register);
            instructions.push(reload.address);
            instructions.push(reload.load);
            operand.virtual_register = reloaded;
        }
        // Edge-transport arguments read at the same end-of-block position,
        // after the terminator instruction executes. Their pairs follow any
        // terminator-operand pairs in successor then binding order; only the
        // argument register moves while the binding keeps its declaration.
        for successor in super::control_successors_mut(&mut terminator)
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
                let reloaded = reload.reload_register.id;
                function.virtual_registers.push(reload.address_register);
                function.virtual_registers.push(reload.reload_register);
                instructions.push(reload.address);
                instructions.push(reload.load);
                let SelectedValueTransport::Registers { argument, .. } = &mut binding.transport
                else {
                    unreachable!()
                };
                *argument = reloaded;
            }
        }
        boundaries.push(
            u32::try_from(instructions.len()).map_err(|_| RuntimeSpillError::IdentityOverflow)?,
        );
        instruction_positions.push(
            u32::try_from(instructions.len()).map_err(|_| RuntimeSpillError::IdentityOverflow)?,
        );
        for settlement in &mut function.boundary_settlements {
            if settlement.block != block.id {
                continue;
            }
            // Eliminated completions retain their gap; hosted settlements name
            // the surviving instruction, after any newly inserted reloads.
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
                .ok_or(RuntimeSpillError::SourceMismatch)?;
        }
        function.blocks[block_index].instructions = instructions;
        function.blocks[block_index].terminator = terminator;
    }
    validate_runtime_spill(
        source,
        function_index,
        register,
        environment,
        budget,
        transformed,
    )
}
