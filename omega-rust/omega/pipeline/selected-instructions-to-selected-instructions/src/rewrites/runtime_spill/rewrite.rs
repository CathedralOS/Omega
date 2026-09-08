use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedInstructionId, SelectedInstructionKind, SelectedLocalStorageSlot, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
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
    let mut instructions = Vec::new();
    let mut boundaries = Vec::new();
    for original in &admitted.function.blocks[0].instructions {
        boundaries.push(
            u32::try_from(instructions.len()).map_err(|_| RuntimeSpillError::IdentityOverflow)?,
        );
        let mut rewritten = original.clone();
        for operand in &mut rewritten.operands {
            if operand.virtual_register != register || original.id == admitted.definition {
                continue;
            }
            let address_instruction =
                SelectedInstructionId(admission::fresh(&mut next_instruction)?);
            let load_instruction = SelectedInstructionId(admission::fresh(&mut next_instruction)?);
            let address_register = VirtualRegisterId(admission::fresh(&mut next_register)?);
            let reload_register = VirtualRegisterId(admission::fresh(&mut next_register)?);
            function.virtual_registers.push(VirtualRegister {
                id: address_register,
                scalar_type: admitted.address_scalar_type,
                class: admitted.victim.class,
                origin: VirtualRegisterOrigin::SpillAddress {
                    instruction: address_instruction,
                    register,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
            function.virtual_registers.push(VirtualRegister {
                id: reload_register,
                scalar_type: admitted.victim.scalar_type,
                class: admitted.victim.class,
                origin: VirtualRegisterOrigin::InstructionResult {
                    instruction: load_instruction,
                    source_value: admitted.source_value,
                },
                definition_site: admitted.victim.definition_site,
                entry_fixed_view: None,
            });
            instructions.push(admission::instruction(
                address_instruction,
                SelectedInstructionKind::FrameAddress {
                    slot: admission::frame(admitted.slot),
                    byte_offset: 0,
                },
                admitted.address,
                &[address_register],
            ));
            instructions.push(admission::instruction(
                load_instruction,
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                admitted.load,
                &[address_register, reload_register],
            ));
            operand.virtual_register = reload_register;
        }
        instructions.push(rewritten);
        if original.id == admitted.definition {
            instructions.push(admission::instruction(
                SelectedInstructionId(admission::fresh(&mut next_instruction)?),
                SelectedInstructionKind::Store64 {
                    slot: admission::frame(admitted.slot),
                    byte_offset: 0,
                },
                admitted.store,
                &[register],
            ));
        }
    }
    boundaries
        .push(u32::try_from(instructions.len()).map_err(|_| RuntimeSpillError::IdentityOverflow)?);
    for settlement in &mut function.boundary_settlements {
        if settlement.block != function.blocks[0].id {
            return Err(RuntimeSpillError::SourceMismatch);
        }
        settlement.instruction_index = *boundaries
            .get(settlement.instruction_index as usize)
            .ok_or(RuntimeSpillError::SourceMismatch)?;
    }
    function.blocks[0].instructions = instructions;
    validate_runtime_spill(
        source,
        function_index,
        register,
        environment,
        budget,
        transformed,
    )
}
