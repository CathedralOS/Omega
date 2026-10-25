use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedValueTransport, VirtualRegisterId};

use super::{RuntimeSplitError, ValidatedRuntimeSplit, admission, shifted_position};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::runtime_spill::{control_mut, control_successors_mut};

/// Split one fixed-view-defined runtime value right after its definition:
/// insert one copy into a fresh register and rebind every later reference to
/// it. The independently checked output is the only admitted result.
pub fn split_selected_runtime_value(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedRuntimeSplit, RuntimeSplitError> {
    let admitted = admission::admit(source, function_index, register, environment, budget)?;
    let fresh = admitted.register.id;
    let definition_index = u32::try_from(admitted.definition_index)
        .map_err(|_| RuntimeSplitError::IdentityOverflow)?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    for &block_index in &admitted.use_blocks {
        let block = &mut function.blocks[block_index];
        for (instruction_index, instruction) in block.instructions.iter_mut().enumerate() {
            if block_index == admitted.block_index && instruction_index <= admitted.definition_index
            {
                continue;
            }
            for operand in &mut instruction.operands {
                if operand.virtual_register == register {
                    operand.virtual_register = fresh;
                }
            }
        }
        for operand in &mut control_mut(&mut block.terminator).operands {
            if operand.virtual_register == register {
                operand.virtual_register = fresh;
            }
        }
        for successor in control_successors_mut(&mut block.terminator)
            .into_iter()
            .flatten()
        {
            for binding in &mut successor.bindings {
                if let SelectedValueTransport::Registers { argument, .. } = &mut binding.transport
                    && *argument == register
                {
                    *argument = fresh;
                }
            }
        }
    }
    let block_id = function.blocks[admitted.block_index].id;
    function.blocks[admitted.block_index]
        .instructions
        .insert(admitted.definition_index + 1, admitted.copy);
    function.virtual_registers.push(admitted.register);
    for settlement in &mut function.boundary_settlements {
        if settlement.block == block_id {
            settlement.instruction_index =
                shifted_position(settlement.instruction_index, definition_index)
                    .ok_or(RuntimeSplitError::IdentityOverflow)?;
        }
    }
    super::validation::seal_runtime_split(source, transformed)
}
