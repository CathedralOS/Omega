use register_model::RegisterOperandAccess;
use selected_instructions::{SelectedInstructionId, SelectedTerminator, VirtualRegisterId};
use semantic_vocabulary::ScalarType;

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
        .find(|block| {
            selected_instructions::terminator_instruction(&block.terminator).id == instruction
        })
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

/// The block owning the fixed-use site — the instruction may be an ordinary
/// block instruction or the block terminator. The operand must still read
/// `source` under `view`.
pub(super) fn replay_site_block(
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
        .find(|block| {
            block.instruction(instruction).is_some()
                || selected_instructions::terminator_instruction(&block.terminator).id
                    == instruction
        })
        .ok_or(FixedViewCopyError::MissingDestination {
            function: function_index,
            instruction: instruction.0,
        })?;
    let site = block
        .instruction(instruction)
        .unwrap_or_else(|| selected_instructions::terminator_instruction(&block.terminator));
    if !site.operands.iter().any(|candidate| {
        candidate.operand == operand
            && candidate.virtual_register == source
            && candidate.access == RegisterOperandAccess::Use
            && candidate.fixed_view == Some(view)
    }) {
        return Err(FixedViewCopyError::MissingDestination {
            function: function_index,
            instruction: instruction.0,
        });
    }
    Ok(block.id)
}

/// Replay applies the same admission the producer's `is_i64` does:
/// `CopyI64` preserves a 64-bit register bit-pattern without interpreting
/// sign, so any 64-bit integer source — signed or unsigned — is admitted.
pub(super) fn replay_is_i64(scalar: ScalarType) -> bool {
    match scalar {
        ScalarType::Integer(integer) => integer.bits() == 64,
        _ => false,
    }
}
