//! End a fixed-use segment at its consumer, without moving the source lifetime.
use super::{RuntimeSplitError, ValidatedRuntimeSplit, admission};
use crate::ValidatedSelectedAnalysis;
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBlockOrigin, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::ScalarType;

impl ValidatedRuntimeSplit {
    /// Split one ordinary untied integer fixed use. The copy immediately
    /// precedes its consumer, so it dominates that one read even in a cyclic
    /// function. Every other reference, incoming binding, structural transport,
    /// source register and CFG edge remains verbatim. No storage is created.
    /// Allocation calls this only for a genuinely empty whole-range domain;
    /// it does not classify that constraint conflict as register pressure.
    pub fn before_fixed_use(
        source: &impl ValidatedSelectedAnalysis,
        function_index: usize,
        register: VirtualRegisterId,
        environment: &ValidatedTargetRegisterEnvironment,
        budget: OptimizationWorkBudget,
    ) -> Result<Self, RuntimeSplitError> {
        let (plan, function) =
            crate::rewrites::plan_function(source, function_index, environment, || {
                RuntimeSplitError::SourceMismatch
            })?;
        let victim = function
            .virtual_register(register)
            .ok_or(RuntimeSplitError::UnsupportedValue)?;
        if !matches!(victim.scalar_type, ScalarType::Integer(integer) if !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64))
        {
            return Err(RuntimeSplitError::UnsupportedValue);
        }
        let source_value = match victim.origin {
            VirtualRegisterOrigin::InstructionResult { source_value, .. }
            | VirtualRegisterOrigin::BlockParameter { source_value, .. }
            | VirtualRegisterOrigin::EntryParameter { source_value, .. } => source_value,
            _ => return Err(RuntimeSplitError::UnsupportedValue),
        };
        if victim.definition_site.is_none() {
            return Err(RuntimeSplitError::UnsupportedValue);
        }
        let (block_index, instruction_index, operand_index) = function
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| {
                matches!(
                    block.origin,
                    SelectedBlockOrigin::Source(_) | SelectedBlockOrigin::Operation { .. }
                )
            })
            .find_map(|(block_index, block)| {
                block.instructions.iter().enumerate().find_map(
                    |(instruction_index, instruction)| {
                        instruction
                            .operands
                            .iter()
                            .position(|operand| {
                                operand.virtual_register == register
                                    && operand.fixed_view.is_some()
                                    && admission::rebindable_use(operand, victim)
                            })
                            .map(|operand| (block_index, instruction_index, operand))
                    },
                )
            })
            .ok_or(RuntimeSplitError::UnsupportedUse)?;
        let consumer = &function.blocks[block_index].instructions[instruction_index];
        let chosen = &consumer.operands[operand_index];
        // A co-operand tied to this read would demand the same physical home:
        // relocating only the read must not sever that relationship.
        if chosen.access != RegisterOperandAccess::Use
            || consumer
                .operands
                .iter()
                .any(|operand| operand.tied_to == Some(chosen.operand))
        {
            return Err(RuntimeSplitError::UnsupportedUse);
        }
        let row = admission::copy_row(environment, victim)?;
        admission::charge(plan, function, 1, budget)?;
        let instruction =
            selected_instructions::SelectedInstructionId(admission::next_instruction(function)?);
        let fresh = VirtualRegisterId(admission::next_register(function)?);
        let copy = admission::copy_instruction(row, instruction, register, fresh, source_value);
        let result = VirtualRegister {
            id: fresh,
            scalar_type: victim.scalar_type,
            class: victim.class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value,
            },
            definition_site: victim.definition_site,
            entry_fixed_view: None,
        };
        let mut transformed = plan.clone();
        let body = &mut transformed.functions[function_index];
        let block_id = body.blocks[block_index].id;
        body.blocks[block_index].instructions[instruction_index].operands[operand_index]
            .virtual_register = fresh;
        body.blocks[block_index]
            .instructions
            .insert(instruction_index, copy);
        body.virtual_registers.push(result);
        let insertion =
            u32::try_from(instruction_index).map_err(|_| RuntimeSplitError::IdentityOverflow)?;
        for settlement in &mut body.boundary_settlements {
            if settlement.block == block_id && settlement.instruction_index >= insertion {
                settlement.instruction_index = settlement
                    .instruction_index
                    .checked_add(1)
                    .ok_or(RuntimeSplitError::IdentityOverflow)?;
            }
        }
        super::validation::seal_runtime_split(source, transformed)
    }
}
