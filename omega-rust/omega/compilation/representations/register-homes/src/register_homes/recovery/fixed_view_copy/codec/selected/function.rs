use selected_instructions::{SelectedBlockId, SelectedFunction};
use semantic_vocabulary::{EdgeId, MachineId, OperationId, StructuralTypeId};

use crate::FixedViewCopyDecodeError;

use super::{
    block::{decode_block, encode_block},
    register::{decode_register, encode_register},
};
use crate::register_homes::recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_id, decode_ids, decode_list, decode_option_u64, encode_ids, encode_option_u64,
    length,
};

pub(super) fn encode_function(
    bytes: &mut Vec<u8>,
    function: &SelectedFunction,
) -> Result<(), terminal_codec::CodecError> {
    crate::codec_bytes::push_encoded(bytes, function.machine.get().to_le_bytes());
    encode_option_u64(bytes, function.attachment.map(|value| value.get()));
    encode_option_u64(
        bytes,
        function.nominal_cleanup_subject.map(|value| value.get()),
    );
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
    crate::codec_bytes::push_encoded(bytes, function.entry_block.0.to_le_bytes());
    length(bytes, function.virtual_registers.len());
    for register in &function.virtual_registers {
        encode_register(bytes, register);
    }
    length(bytes, function.blocks.len());
    for block in &function.blocks {
        encode_block(bytes, block);
    }
    super::structural::encode_contracts(bytes, function)
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
    let nominal_cleanup_subject = match decode_option_u64(cursor)? {
        None => None,
        Some(raw) => Some(
            StructuralTypeId::new(raw).ok_or(FixedViewCopyDecodeError::InvalidSemanticId(raw))?,
        ),
    };
    let operations = decode_ids(cursor, OperationId::new)?;
    let edges = decode_ids(cursor, EdgeId::new)?;
    let entry_block = SelectedBlockId(cursor.u32()?);
    let virtual_registers = decode_list(cursor, decode_register)?;
    let blocks = decode_list(cursor, decode_block)?;
    let mut function = SelectedFunction {
        machine,
        attachment,
        nominal_cleanup_subject,
        provenance: target_operations::TerminalPsiProvenance { operations, edges },
        entry_block,
        virtual_registers,
        blocks: blocks.into(),
        structural: None,
        outgoing_arguments: Vec::new(),
        local_storage_slots: Vec::new(),
        calls: Vec::new(),
        normalized_foreign_calls: Vec::new(),
        memory_accesses: Vec::new(),
        boundary_settlements: Vec::new(),
    };
    super::structural::decode_contracts(cursor, &mut function)?;
    Ok(function)
}
