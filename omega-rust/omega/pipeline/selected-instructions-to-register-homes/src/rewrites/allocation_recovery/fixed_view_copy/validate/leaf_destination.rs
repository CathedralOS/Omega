use register_model::RegisterOperandAccess;
use selected_instructions::{SelectedInstructionId, SelectedTerminator, VirtualRegisterId};
use semantic_vocabulary::{IntegerSign, ScalarType};

use crate::FixedViewCopyError;

pub(super) fn replay_leaf_block(
    function_index: usize,
    function: &selected_instructions::SelectedFunction,
    instruction: SelectedInstructionId,
    operand: u16,
    source: VirtualRegisterId,
    view: register_model::RegisterViewId,
) -> Result<selected_instructions::SelectedBlockId, FixedViewCopyError> {
    let block = function
        .blocks
        .iter()
        .find(|block| terminator(&block.terminator).id == instruction)
        .ok_or(FixedViewCopyError::MissingDestination {
            function: function_index,
            instruction: instruction.0,
        })?;
    let SelectedTerminator::Return {
        instruction: destination,
        ..
    } = &block.terminator
    else {
        return Err(FixedViewCopyError::NonLeafDestination {
            function: function_index,
            instruction: instruction.0,
        });
    };
    if block.id == function.entry_block
        || !destination.operands.iter().any(|candidate| {
            candidate.operand == operand
                && candidate.virtual_register == source
                && candidate.access == RegisterOperandAccess::Use
                && candidate.fixed_view == Some(view)
        })
    {
        return Err(FixedViewCopyError::MissingDestination {
            function: function_index,
            instruction: instruction.0,
        });
    }
    Ok(block.id)
}

pub(super) fn terminator(
    terminator: &SelectedTerminator,
) -> &selected_instructions::SelectedInstruction {
    match terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}

pub(super) fn replay_is_u64(scalar: ScalarType) -> bool {
    match scalar {
        ScalarType::Integer(integer) => {
            integer.sign() == IntegerSign::Unsigned && integer.bits() == 64
        }
        _ => false,
    }
}
