use omega_selected_instructions::{SelectedBlockId, SelectedFunction};
use psi_core::{EdgeId, MachineId, OperationId, StructuralTypeId};

use crate::FixedViewCopyDecodeError;

use super::{
    block::{decode_block, encode_block},
    register::{decode_register, encode_register},
};
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_id, decode_ids, decode_option_u64, encode_ids, encode_option_u64, length,
};

pub(super) fn encode_function(bytes: &mut Vec<u8>, function: &SelectedFunction) {
    bytes.extend_from_slice(&function.machine.get().to_le_bytes());
    encode_option_u64(bytes, function.attachment.map(|value| value.get()));
    encode_ids(
        bytes,
        function
            .provenance
            .operations
            .iter()
            .map(|value| value.get()),
    );
    encode_ids(
        bytes,
        function.provenance.edges.iter().map(|value| value.get()),
    );
    bytes.extend_from_slice(&function.entry_block.0.to_le_bytes());
    length(bytes, function.virtual_registers.len());
    for register in &function.virtual_registers {
        encode_register(bytes, register);
    }
    length(bytes, function.blocks.len());
    for block in &function.blocks {
        encode_block(bytes, block);
    }
}

pub(super) fn decode_function(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedFunction, FixedViewCopyDecodeError> {
    let machine = decode_id(cursor, MachineId::new)?;
    let attachment = match decode_option_u64(cursor)? {
        None => None,
        Some(raw) => Some(
            StructuralTypeId::new(raw).ok_or(FixedViewCopyDecodeError::InvalidSemanticId(raw))?,
        ),
    };
    let operations = decode_ids(cursor, OperationId::new)?;
    let edges = decode_ids(cursor, EdgeId::new)?;
    let entry_block = SelectedBlockId(cursor.u32()?);
    let register_count = cursor.length()?;
    let mut virtual_registers = Vec::with_capacity(register_count.min(cursor.remaining()));
    for _ in 0..register_count {
        virtual_registers.push(decode_register(cursor)?);
    }
    let block_count = cursor.length()?;
    let mut blocks = Vec::with_capacity(block_count.min(cursor.remaining()));
    for _ in 0..block_count {
        blocks.push(decode_block(cursor)?);
    }
    Ok(SelectedFunction {
        machine,
        attachment,
        provenance: omega_target_operations::TerminalPsiProvenance { operations, edges },
        entry_block,
        virtual_registers,
        blocks,
    })
}
