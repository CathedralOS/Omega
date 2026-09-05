use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedOperand,
    SelectedTerminator, VirtualRegisterId,
};

use crate::PressureRematerializationError;

pub(super) fn operand(
    constraint: &register_model::RegisterOperandConstraint,
    register: VirtualRegisterId,
) -> SelectedOperand {
    SelectedOperand {
        operand: constraint.operand,
        virtual_register: register,
        access: constraint.access,
        class: constraint.class,
        fixed_view: constraint.fixed_view,
        tied_to: constraint.tied_to,
        early_clobber: constraint.early_clobber,
    }
}

pub(super) fn find_instruction(
    block: &selected_instructions::SelectedBlock,
    id: SelectedInstructionId,
) -> Option<&SelectedInstruction> {
    block
        .instructions
        .iter()
        .find(|instruction| instruction.id == id)
        .or_else(|| match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Return { instruction, .. }
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
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id.0)
                .chain(std::iter::once(match &block.terminator {
                    SelectedTerminator::ConditionalBranch { instruction, .. }
                    | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
                    | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
                    | SelectedTerminator::Return { instruction, .. } => instruction.id.0,
                }))
        })
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
    let count = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len().checked_add(1)?)
        })
        .ok_or(PressureRematerializationError::IdentifierOverflow {
            function: function_index,
        })?;
    u32::try_from(count).map_err(|_| PressureRematerializationError::IdentifierOverflow {
        function: function_index,
    })
}
