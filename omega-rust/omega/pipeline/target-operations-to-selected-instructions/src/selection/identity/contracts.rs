use super::*;
use selected_instructions::{OutgoingArgumentSlotId, SelectedMemoryAccessRole};
fn blob(bytes: &mut Vec<u8>, content: &[u8]) {
    encode_len(bytes, content.len());
    bytes.extend_from_slice(content);
}
pub(super) fn slot(bytes: &mut Vec<u8>, slot: OutgoingArgumentSlotId) {
    bytes.extend_from_slice(&slot.operation.get().to_le_bytes());
    bytes.extend_from_slice(&slot.argument_index.to_le_bytes());
}
fn local_slot(bytes: &mut Vec<u8>, slot: selected_instructions::LocalStorageSlotId) {
    slot.encode_identity(bytes);
}
pub(super) fn frame_slot(bytes: &mut Vec<u8>, slot: selected_instructions::FrameStorageSlotId) {
    match slot {
        selected_instructions::FrameStorageSlotId::Incoming {
            parameter_index,
            abi_stack_byte_offset,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&parameter_index.to_le_bytes());
            bytes.extend_from_slice(&abi_stack_byte_offset.to_le_bytes());
        }
        selected_instructions::FrameStorageSlotId::Outgoing(value) => {
            bytes.push(0);
            self::slot(bytes, value);
        }
        selected_instructions::FrameStorageSlotId::Local(value) => {
            bytes.push(1);
            local_slot(bytes, value);
        }
    }
}
pub(super) fn encode(bytes: &mut Vec<u8>, function: &SelectedFunction) {
    match &function.ranked {
        None => bytes.push(0),
        Some(custody) => {
            match abstract_operations::ranked_u32_countdown_custody_identity(custody) {
                Ok(identity) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&identity);
                }
                // Invalid raw proposals have no proof authority and never decode as admitted custody.
                Err(_) => bytes.push(2),
            }
        }
    }
    match &function.structural {
        Some(contract) => {
            bytes.push(1);
            blob(bytes, &contract.canonical_bytes());
        }
        None => bytes.push(0),
    }
    encode_len(bytes, function.outgoing_arguments.len());
    for row in &function.outgoing_arguments {
        slot(bytes, row.id);
        bytes.extend_from_slice(&row.byte_size.to_le_bytes());
        bytes.extend_from_slice(&row.alignment.to_le_bytes());
        bytes.extend_from_slice(&row.abi_stack_byte_offset.to_le_bytes());
    }
    encode_len(bytes, function.local_storage_slots.len());
    for row in &function.local_storage_slots {
        local_slot(bytes, row.id);
        bytes.extend_from_slice(&row.byte_size.to_le_bytes());
        bytes.extend_from_slice(&row.alignment.to_le_bytes());
    }
    encode_len(bytes, function.calls.len());
    for row in &function.calls {
        bytes.extend_from_slice(&row.instruction.0.to_le_bytes());
        bytes.extend_from_slice(&row.operation.get().to_le_bytes());
        blob(
            bytes,
            &row.call
                .canonical_bytes_with_effects(row.effect, &row.ownership),
        );
    }
    encode_len(bytes, function.memory_accesses.len());
    for row in &function.memory_accesses {
        bytes.extend_from_slice(&row.instruction.0.to_le_bytes());
        row.origin.encode_identity(bytes);
        bytes.extend_from_slice(&row.place.get().to_le_bytes());
        bytes.extend_from_slice(&row.byte_offset.to_le_bytes());
        bytes.extend_from_slice(&row.byte_count.to_le_bytes());
        match row.role {
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
            SelectedMemoryAccessRole::ReadPlace => bytes.push(0),
            SelectedMemoryAccessRole::WritePlace => bytes.push(6),
            SelectedMemoryAccessRole::WriteOutgoing { slot: value } => {
                bytes.push(1);
                slot(bytes, value);
            }
            SelectedMemoryAccessRole::AddressOutgoing { slot: value } => {
                bytes.push(2);
                slot(bytes, value);
            }
            SelectedMemoryAccessRole::WriteLocal { slot: value } => {
                bytes.push(4);
                local_slot(bytes, value);
            }
            SelectedMemoryAccessRole::AddressLocal { slot: value } => {
                bytes.push(5);
                local_slot(bytes, value);
            }
        }
    }
    encode_len(bytes, function.boundary_settlements.len());
    for row in &function.boundary_settlements {
        bytes.extend_from_slice(&row.block.0.to_le_bytes());
        bytes.extend_from_slice(&row.instruction_index.to_le_bytes());
        blob(bytes, &row.settlement.canonical_bytes());
    }
}
