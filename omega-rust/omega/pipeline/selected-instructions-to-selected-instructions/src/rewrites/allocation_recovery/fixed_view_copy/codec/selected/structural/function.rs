//! Complete instruction-keyed metadata for the ordinary function graph.
use super::{call::*, normalized_foreign::*, settlements::*, signature::*};
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
    if tag == 4 {
        return Ok(
            selected_instructions::LocalStorageSlotId::StructuralParameter {
                place: decode_id(cursor, PlaceId::new)?,
            },
        );
    }
    if tag == 3 {
        return Ok(
            selected_instructions::LocalStorageSlotId::StructuralBlockParameter {
                block: decode_id(cursor, semantic_vocabulary::BlockId::new)?,
                place: decode_id(cursor, PlaceId::new)?,
            },
        );
    }
    if tag == 2 {
        return Ok(selected_instructions::LocalStorageSlotId::Spill {
            register: selected_instructions::VirtualRegisterId(cursor.u32()?),
        });
    }
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
    slot.encode_identity(bytes);
}
fn decode_slot(
    cursor: &mut Cursor<'_>,
) -> Result<OutgoingArgumentSlotId, FixedViewCopyDecodeError> {
    Ok(OutgoingArgumentSlotId {
        operation: decode_id(cursor, OperationId::new)?,
        argument_index: cursor.u32()?,
        role: match cursor.byte()? {
            0 => selected_instructions::OutgoingArgumentSlotRole::Argument,
            1 => selected_instructions::OutgoingArgumentSlotRole::ValueCopy,
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        },
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
    length(bytes, function.normalized_foreign_calls.len());
    for row in &function.normalized_foreign_calls {
        encode_normalized_foreign_call(bytes, row);
    }
    length(bytes, function.memory_accesses.len());
    for row in &function.memory_accesses {
        bytes.extend_from_slice(&row.instruction.0.to_le_bytes());
        row.origin.encode_identity(bytes);
        bytes.extend_from_slice(&row.place.get().to_le_bytes());
        bytes.extend_from_slice(&row.byte_offset.to_le_bytes());
        bytes.extend_from_slice(&row.byte_count.to_le_bytes());
        match row.role {
            SelectedMemoryAccessRole::ReadByteSpan {
                length,
                obligation,
                accepted_fact,
            }
            | SelectedMemoryAccessRole::WriteByteSpan {
                length,
                obligation,
                accepted_fact,
            } => {
                bytes.push(
                    if matches!(row.role, SelectedMemoryAccessRole::ReadByteSpan { .. }) {
                        8
                    } else {
                        9
                    },
                );
                bytes.extend_from_slice(&length.get().to_le_bytes());
                bytes.extend_from_slice(&obligation.get().to_le_bytes());
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::WriteByteSequence {
                index,
                value,
                length,
                obligation,
                accepted_fact,
            } => {
                bytes.push(7);
                for identity in [index.get(), value.get(), length.get(), obligation.get()] {
                    bytes.extend_from_slice(&identity.to_le_bytes());
                }
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
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
            SelectedMemoryAccessRole::WritePlace => bytes.push(6),
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
        function
            .normalized_foreign_calls
            .push(decode_normalized_foreign_call(cursor)?);
    }
    let count = cursor.length()?;
    for _ in 0..count {
        let instruction = SelectedInstructionId(cursor.u32()?);
        let origin = match cursor.byte()? {
            0 => selected_instructions::SelectedMemoryAccessOrigin::Operation(decode_id(
                cursor,
                OperationId::new,
            )?),
            1 => selected_instructions::SelectedMemoryAccessOrigin::Edge(decode_id(
                cursor,
                semantic_vocabulary::EdgeId::new,
            )?),
            2 => selected_instructions::SelectedMemoryAccessOrigin::Block(decode_id(
                cursor,
                semantic_vocabulary::BlockId::new,
            )?),
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        };
        let place = decode_id(cursor, PlaceId::new)?;
        let byte_offset = cursor.u32()?;
        let byte_count = cursor.u32()?;
        let role = match cursor.byte()? {
            tag @ (8 | 9) => {
                let length = decode_id(cursor, semantic_vocabulary::ValueId::new)?;
                let obligation = decode_id(cursor, semantic_vocabulary::ObligationId::new)?;
                let accepted_fact =
                    optimization_core::AcceptedObligationFactIdentity::from_bytes(cursor.array()?);
                if tag == 8 {
                    SelectedMemoryAccessRole::ReadByteSpan {
                        length,
                        obligation,
                        accepted_fact,
                    }
                } else {
                    SelectedMemoryAccessRole::WriteByteSpan {
                        length,
                        obligation,
                        accepted_fact,
                    }
                }
            }
            7 => SelectedMemoryAccessRole::WriteByteSequence {
                index: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                value: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                length: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                obligation: decode_id(cursor, semantic_vocabulary::ObligationId::new)?,
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    cursor.array()?,
                ),
            },
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
            6 => SelectedMemoryAccessRole::WritePlace,
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
            origin,
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

#[cfg(test)]
mod local_slot_tests {
    use super::{
        Cursor, FixedViewCopyDecodeError, OperationId, PlaceId, SelectedBlockId, SelectedFunction,
        SelectedInstructionId, SelectedMemoryAccess, SelectedMemoryAccessRole,
    };
    use crate::codec::selected::structural::decode_contracts;
    use crate::codec::selected::structural::decode_local_slot;
    use crate::codec::selected::structural::encode_contracts;

    #[test]
    fn byte_span_contract_roundtrip_retains_length_obligation_and_accepted_fact() {
        let empty = || SelectedFunction {
            machine: semantic_vocabulary::MachineId::new(1).unwrap(),
            attachment: None,
            provenance: Default::default(),
            structural: None,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            normalized_foreign_calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers: Vec::new(),
            blocks: Vec::new(),
        };
        let length = semantic_vocabulary::ValueId::new(7).unwrap();
        let obligation = semantic_vocabulary::ObligationId::new(8).unwrap();
        let accepted_fact = optimization_core::AcceptedObligationFactIdentity::from_bytes([9; 32]);
        let mut source = empty();
        for role in [
            SelectedMemoryAccessRole::ReadByteSpan {
                length,
                obligation,
                accepted_fact,
            },
            SelectedMemoryAccessRole::WriteByteSpan {
                length,
                obligation,
                accepted_fact,
            },
        ] {
            source.memory_accesses.push(SelectedMemoryAccess {
                instruction: SelectedInstructionId(2),
                origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
                    OperationId::new(3).unwrap(),
                ),
                place: PlaceId::new(4).unwrap(),
                byte_offset: 5,
                byte_count: 0,
                role,
            });
        }
        let mut bytes = Vec::new();
        encode_contracts(&mut bytes, &source);
        let mut cursor = Cursor::new(&bytes);
        let mut decoded = empty();
        decode_contracts(&mut cursor, &mut decoded).unwrap();
        assert_eq!(cursor.remaining(), 0);
        assert_eq!(decoded, source);
        for mutation in 0..3 {
            let mut changed = source.clone();
            let SelectedMemoryAccessRole::WriteByteSpan {
                length,
                obligation,
                accepted_fact,
            } = &mut changed.memory_accesses[1].role
            else {
                panic!("write span");
            };
            match mutation {
                0 => *length = semantic_vocabulary::ValueId::new(17).unwrap(),
                1 => *obligation = semantic_vocabulary::ObligationId::new(18).unwrap(),
                _ => {
                    *accepted_fact =
                        optimization_core::AcceptedObligationFactIdentity::from_bytes([19; 32])
                }
            }
            let mut changed_bytes = Vec::new();
            encode_contracts(&mut changed_bytes, &changed);
            assert_ne!(bytes, changed_bytes);
        }
        for length in 0..bytes.len() {
            assert!(decode_contracts(&mut Cursor::new(&bytes[..length]), &mut empty()).is_err());
        }
    }

    #[test]
    fn owned_entry_slot_decoder_rejects_zero_truncation_and_unknown_tag() {
        let encoded = [4, 1, 0, 0, 0, 0, 0, 0, 0];
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(
            decode_local_slot(&mut cursor).unwrap(),
            selected_instructions::LocalStorageSlotId::StructuralParameter {
                place: PlaceId::new(1).unwrap(),
            }
        );
        assert_eq!(cursor.remaining(), 0);
        for length in 0..encoded.len() {
            assert!(decode_local_slot(&mut Cursor::new(&encoded[..length])).is_err());
        }
        let mut zero_place = encoded;
        zero_place[1] = 0;
        assert!(decode_local_slot(&mut Cursor::new(&zero_place)).is_err());
        let mut unknown_tag = encoded;
        unknown_tag[0] = 5;
        assert_eq!(
            decode_local_slot(&mut Cursor::new(&unknown_tag)),
            Err(FixedViewCopyDecodeError::UnknownOption(5))
        );
    }
}
