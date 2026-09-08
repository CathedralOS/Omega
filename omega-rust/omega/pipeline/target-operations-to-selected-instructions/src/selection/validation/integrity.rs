use crate::selection::constraints::row;
use crate::selection::shared::*;

pub(super) fn validate_block_constraints(
    function_index: usize,
    block: &SelectedBlock,
    function: &SelectedFunction,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    for instruction in block
        .instructions
        .iter()
        .chain(std::iter::once(terminator_instruction(&block.terminator)))
    {
        let row = row(catalog, instruction.constraint)?;
        if instruction.operands.len() != row.operands.len() {
            return Err(SelectedInstructionError::ConstraintOperandMismatch {
                function: function_index,
                instruction: instruction.id.0,
            });
        }
        for (operand, constraint) in instruction.operands.iter().zip(&row.operands) {
            let Some(register) = function
                .virtual_registers
                .get(operand.virtual_register.0 as usize)
            else {
                return Err(SelectedInstructionError::ConstraintOperandMismatch {
                    function: function_index,
                    instruction: instruction.id.0,
                });
            };
            if operand.operand != constraint.operand
                || operand.access != constraint.access
                || operand.class != constraint.class
                || operand.fixed_view != constraint.fixed_view
                || operand.tied_to != constraint.tied_to
                || operand.early_clobber != constraint.early_clobber
                || register.class != constraint.class
            {
                return Err(SelectedInstructionError::ConstraintOperandMismatch {
                    function: function_index,
                    instruction: instruction.id.0,
                });
            }
        }
        if instruction.implicit_uses != row.implicit_uses
            || instruction.implicit_defs != row.implicit_defs
            || instruction.clobbers != row.clobbers
        {
            return Err(SelectedInstructionError::ConstraintEffectMismatch {
                function: function_index,
                instruction: instruction.id.0,
            });
        }
    }
    Ok(())
}

pub(super) use super::def_use::validate_def_use;

pub(super) fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::Return { instruction, .. }
        | SelectedTerminator::HostedExitProcess { instruction, .. } => instruction,
    }
}
