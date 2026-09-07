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
pub(super) fn encode(bytes: &mut Vec<u8>, function: &SelectedFunction) {
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
        bytes.extend_from_slice(&row.operation.get().to_le_bytes());
        bytes.extend_from_slice(&row.place.get().to_le_bytes());
        bytes.extend_from_slice(&row.byte_offset.to_le_bytes());
        bytes.extend_from_slice(&row.byte_count.to_le_bytes());
        match row.role {
            SelectedMemoryAccessRole::ReadPlace => bytes.push(0),
            SelectedMemoryAccessRole::WriteOutgoing { slot: value } => {
                bytes.push(1);
                slot(bytes, value);
            }
            SelectedMemoryAccessRole::AddressOutgoing { slot: value } => {
                bytes.push(2);
                slot(bytes, value);
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
