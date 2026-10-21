use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedTerminator,
};

use crate::PressureRematerializationError;
use crate::rewrites::block_edges::block_instruction_ids;

pub(super) use crate::rewrites::selected_operands::selected_operand as operand;

pub(super) fn find_instruction(
    block: &selected_instructions::SelectedBlock,
    id: SelectedInstructionId,
) -> Option<&SelectedInstruction> {
    block.instruction(id).or_else(|| match &block.terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | selected_instructions::terminator_no_successor_pattern!(instruction, ..)
            if instruction.id == id =>
        {
            Some(instruction)
        }
        _ => None,
    })
}

pub(super) fn validate_dense(
    function_index: usize,
    function: &SelectedFunction,
) -> Result<(), PressureRematerializationError> {
    if function
        .virtual_registers
        .iter()
        .enumerate()
        .any(|(index, register)| usize::try_from(register.id.0) != Ok(index))
    {
        return Err(PressureRematerializationError::FunctionMismatch {
            function: function_index,
        });
    }
    let count = instruction_count(function_index, function)?;
    let mut ids = function
        .blocks
        .iter()
        .flat_map(block_instruction_ids)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    if ids != (0..count).collect::<Vec<_>>() {
        return Err(PressureRematerializationError::FunctionMismatch {
            function: function_index,
        });
    }
    Ok(())
}

pub(super) fn instruction_count(
    function_index: usize,
    function: &SelectedFunction,
) -> Result<u32, PressureRematerializationError> {
    let count = crate::rewrites::function_scan(function).ok_or(
        PressureRematerializationError::IdentifierOverflow {
            function: function_index,
        },
    )?;
    u32::try_from(count).map_err(|_| PressureRematerializationError::IdentifierOverflow {
        function: function_index,
    })
}
