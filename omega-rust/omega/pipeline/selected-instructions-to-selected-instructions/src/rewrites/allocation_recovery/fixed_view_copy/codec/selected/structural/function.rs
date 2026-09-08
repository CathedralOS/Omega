//! Complete instruction-keyed metadata for the ordinary function graph.
use super::{call::*, settlements::*, signature::*};
use crate::FixedViewCopyDecodeError;
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_id, length,
};
use selected_instructions::{
    OutgoingArgumentSlotId, SelectedBlockId, SelectedBoundarySettlement, SelectedFunction,
    SelectedInstructionId, SelectedMemoryAccess, SelectedMemoryAccessRole,
    SelectedOutgoingArgumentSlot,
};
use semantic_vocabulary::{OperationId, PlaceId};
pub(in crate::rewrites::allocation_recovery::fixed_view_copy::codec::selected) fn decode_local_slot(
    cursor: &mut Cursor<'_>,
) -> Result<selected_instructions::LocalStorageSlotId, FixedViewCopyDecodeError> {
    let tag = cursor.byte()?;
    let operation = decode_id(cursor, OperationId::new)?;
    match tag {
        0 => Ok(selected_instructions::LocalStorageSlotId::Structural {
            operation,
            place: decode_id(cursor, PlaceId::new)?,
        }),
        1 => Ok(selected_instructions::LocalStorageSlotId::Boundary { operation }),
        tag => Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    }
}

fn encode_slot(bytes: &mut Vec<u8>, slot: OutgoingArgumentSlotId) {
    bytes.extend_from_slice(&slot.operation.get().to_le_bytes());
    bytes.extend_from_slice(&slot.argument_index.to_le_bytes());
}
fn decode_slot(
    cursor: &mut Cursor<'_>,
) -> Result<OutgoingArgumentSlotId, FixedViewCopyDecodeError> {
    Ok(OutgoingArgumentSlotId {
        operation: decode_id(cursor, OperationId::new)?,
        argument_index: cursor.u32()?,
    })
}
pub(in crate::rewrites::allocation_recovery::fixed_view_copy::codec::selected) fn encode_contracts(
    bytes: &mut Vec<u8>,
    function: &SelectedFunction,
) {
    match &function.structural {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            encode_signature(bytes, value);
        }
    }
    length(bytes, function.local_storage_slots.len());
    for slot in &function.local_storage_slots {
        slot.id.encode_identity(bytes);
        bytes.extend_from_slice(&slot.byte_size.to_le_bytes());
        bytes.extend_from_slice(&slot.alignment.to_le_bytes());
    }
    length(bytes, function.outgoing_arguments.len());
    for row in &function.outgoing_arguments {
        encode_slot(bytes, row.id);
        bytes.extend_from_slice(&row.byte_size.to_le_bytes());
        bytes.extend_from_slice(&row.alignment.to_le_bytes());
        bytes.extend_from_slice(&row.abi_stack_byte_offset.to_le_bytes());
    }
    length(bytes, function.calls.len());
    for row in &function.calls {
        encode_call(bytes, row);
    }
    length(bytes, function.memory_accesses.len());
    for row in &function.memory_accesses {
        bytes.extend_from_slice(&row.instruction.0.to_le_bytes());
        bytes.extend_from_slice(&row.operation.get().to_le_bytes());
        bytes.extend_from_slice(&row.place.get().to_le_bytes());
        bytes.extend_from_slice(&row.byte_offset.to_le_bytes());
        bytes.extend_from_slice(&row.byte_count.to_le_bytes());
        match row.role {
            SelectedMemoryAccessRole::ReadByteSequence {
                index,
                length,
                obligation,
                accepted_fact,
            } => {
                bytes.push(3);
                bytes.extend_from_slice(&index.get().to_le_bytes());
                bytes.extend_from_slice(&length.get().to_le_bytes());
                bytes.extend_from_slice(&obligation.get().to_le_bytes());
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::WriteLocal { slot }
            | SelectedMemoryAccessRole::AddressLocal { slot } => {
                bytes.push(
                    if matches!(row.role, SelectedMemoryAccessRole::WriteLocal { .. }) {
                        4
                    } else {
                        5
                    },
                );
                slot.encode_identity(bytes);
            }
            SelectedMemoryAccessRole::ReadPlace => bytes.push(0),
            SelectedMemoryAccessRole::WriteOutgoing { slot } => {
                bytes.push(1);
                encode_slot(bytes, slot);
            }
            SelectedMemoryAccessRole::AddressOutgoing { slot } => {
                bytes.push(2);
                encode_slot(bytes, slot);
            }
        }
    }
    length(bytes, function.boundary_settlements.len());
    for row in &function.boundary_settlements {
        bytes.extend_from_slice(&row.block.0.to_le_bytes());
        bytes.extend_from_slice(&row.instruction_index.to_le_bytes());
        encode_boundary_settlement(bytes, &row.settlement);
    }
}
pub(in crate::rewrites::allocation_recovery::fixed_view_copy::codec::selected) fn decode_contracts(
    cursor: &mut Cursor<'_>,
    function: &mut SelectedFunction,
) -> Result<(), FixedViewCopyDecodeError> {
    function.structural = match cursor.byte()? {
        0 => None,
        1 => Some(decode_signature(cursor)?),
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    let local_count = cursor.length()?;
    for _ in 0..local_count {
        function
            .local_storage_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: decode_local_slot(cursor)?,
                byte_size: cursor.u32()?,
                alignment: cursor.u16()?,
            });
    }
    let count = cursor.length()?;
    for _ in 0..count {
        function
            .outgoing_arguments
            .push(SelectedOutgoingArgumentSlot {
                id: decode_slot(cursor)?,
                byte_size: cursor.u32()?,
                alignment: cursor.u16()?,
                abi_stack_byte_offset: cursor.u32()?,
            });
    }
    let count = cursor.length()?;
    for _ in 0..count {
        function.calls.push(decode_call(cursor)?);
    }
    let count = cursor.length()?;
    for _ in 0..count {
        let instruction = SelectedInstructionId(cursor.u32()?);
        let operation = decode_id(cursor, OperationId::new)?;
        let place = decode_id(cursor, PlaceId::new)?;
        let byte_offset = cursor.u32()?;
        let byte_count = cursor.u32()?;
        let role = match cursor.byte()? {
            3 => SelectedMemoryAccessRole::ReadByteSequence {
                index: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                length: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                obligation: decode_id(cursor, semantic_vocabulary::ObligationId::new)?,
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    cursor.array()?,
                ),
            },
            tag @ (4 | 5) => {
                let slot = decode_local_slot(cursor)?;
                if tag == 4 {
                    SelectedMemoryAccessRole::WriteLocal { slot }
                } else {
                    SelectedMemoryAccessRole::AddressLocal { slot }
                }
            }
            0 => SelectedMemoryAccessRole::ReadPlace,
            1 => SelectedMemoryAccessRole::WriteOutgoing {
                slot: decode_slot(cursor)?,
            },
            2 => SelectedMemoryAccessRole::AddressOutgoing {
                slot: decode_slot(cursor)?,
            },
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        };
        function.memory_accesses.push(SelectedMemoryAccess {
            instruction,
            operation,
            place,
            byte_offset,
            byte_count,
            role,
        });
    }
    let count = cursor.length()?;
    for _ in 0..count {
        function
            .boundary_settlements
            .push(SelectedBoundarySettlement {
                block: SelectedBlockId(cursor.u32()?),
                instruction_index: cursor.u32()?,
                settlement: decode_boundary_settlement(cursor)?,
            });
    }
    Ok(())
}
