use omega_abstract_operations::ValueBinding;
use omega_selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedSuccessor, SelectedTerminator,
};
use psi_core::{BlockId, EdgeId, ValueId};

use crate::FixedViewCopyDecodeError;

use super::{
    instruction::{decode_instruction, encode_instruction},
    provenance::{decode_fuel, encode_fuel},
};
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::{
    primitives::{Cursor, decode_id, length},
    values::{decode_scalar, encode_scalar},
};

pub(super) fn encode_block(bytes: &mut Vec<u8>, block: &SelectedBlock) {
    bytes.extend_from_slice(&block.id.0.to_le_bytes());
    bytes.extend_from_slice(&block.source_block.get().to_le_bytes());
    length(bytes, block.instructions.len());
    for instruction in &block.instructions {
        encode_instruction(bytes, instruction);
    }
    match &block.terminator {
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => {
            bytes.push(0);
            encode_instruction(bytes, instruction);
            encode_successor(bytes, when_nonzero);
            encode_successor(bytes, when_zero);
        }
        SelectedTerminator::Return {
            instruction,
            psi_return_edge,
        } => {
            bytes.push(1);
            encode_instruction(bytes, instruction);
            bytes.extend_from_slice(&psi_return_edge.get().to_le_bytes());
        }
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => {
            bytes.push(2);
            encode_instruction(bytes, instruction);
            encode_successor(bytes, when_less);
            encode_successor(bytes, when_not_less);
        }
        SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => {
            bytes.push(3);
            encode_instruction(bytes, instruction);
            encode_successor(bytes, when_less);
            encode_successor(bytes, when_not_less);
        }
    }
}

pub(super) fn decode_block(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedBlock, FixedViewCopyDecodeError> {
    let id = SelectedBlockId(cursor.u32()?);
    let source_block = decode_id(cursor, BlockId::new)?;
    let instruction_count = cursor.length()?;
    let mut instructions = Vec::with_capacity(instruction_count.min(cursor.remaining()));
    for _ in 0..instruction_count {
        instructions.push(decode_instruction(cursor)?);
    }
    let terminator = match cursor.byte()? {
        0 => SelectedTerminator::ConditionalBranch {
            instruction: decode_instruction(cursor)?,
            when_nonzero: decode_successor(cursor)?,
            when_zero: decode_successor(cursor)?,
        },
        1 => SelectedTerminator::Return {
            instruction: decode_instruction(cursor)?,
            psi_return_edge: decode_id(cursor, EdgeId::new)?,
        },
        2 => SelectedTerminator::ConditionalBranchU64LessThan {
            instruction: decode_instruction(cursor)?,
            when_less: decode_successor(cursor)?,
            when_not_less: decode_successor(cursor)?,
        },
        3 => SelectedTerminator::ConditionalBranchI64LessThan {
            instruction: decode_instruction(cursor)?,
            when_less: decode_successor(cursor)?,
            when_not_less: decode_successor(cursor)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownTerminator(tag)),
    };
    Ok(SelectedBlock {
        id,
        source_block,
        instructions,
        terminator,
    })
}

fn encode_successor(bytes: &mut Vec<u8>, successor: &SelectedSuccessor) {
    bytes.extend_from_slice(&successor.psi_edge.get().to_le_bytes());
    bytes.extend_from_slice(&successor.block.0.to_le_bytes());
    bytes.extend_from_slice(&successor.source_target.get().to_le_bytes());
    length(bytes, successor.bindings.len());
    for binding in &successor.bindings {
        bytes.extend_from_slice(&binding.parameter.get().to_le_bytes());
        bytes.extend_from_slice(&binding.argument.get().to_le_bytes());
        encode_scalar(bytes, binding.scalar_type);
    }
    encode_fuel(bytes, &successor.fuel);
}

fn decode_successor(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedSuccessor, FixedViewCopyDecodeError> {
    let psi_edge = decode_id(cursor, EdgeId::new)?;
    let block = SelectedBlockId(cursor.u32()?);
    let source_target = decode_id(cursor, BlockId::new)?;
    let count = cursor.length()?;
    let mut bindings = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        bindings.push(ValueBinding {
            parameter: decode_id(cursor, ValueId::new)?,
            argument: decode_id(cursor, ValueId::new)?,
            scalar_type: decode_scalar(cursor)?,
        });
    }
    Ok(SelectedSuccessor {
        psi_edge,
        block,
        source_target,
        bindings,
        fuel: decode_fuel(cursor)?,
    })
}
