use crate::SelectedFunction;
use crate::codec_bytes::encode_len;
use crate::{OutgoingArgumentSlotId, SelectedMemoryAccessRole};
fn blob(bytes: &mut Vec<u8>, content: &[u8]) {
    encode_len(bytes, content.len());
    bytes.extend_from_slice(content);
}
pub(super) fn slot(bytes: &mut Vec<u8>, slot: OutgoingArgumentSlotId) {
    slot.encode_identity(bytes);
}
fn local_slot(bytes: &mut Vec<u8>, slot: crate::LocalStorageSlotId) {
    slot.encode_identity(bytes);
}
pub(super) fn encode_local_storage_slot(
    bytes: &mut Vec<u8>,
    row: &crate::SelectedLocalStorageSlot,
) {
    local_slot(bytes, row.id);
    crate::codec_bytes::push_encoded(bytes, row.byte_size.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, row.alignment.to_le_bytes());
}
pub(super) fn frame_slot(bytes: &mut Vec<u8>, slot: crate::FrameStorageSlotId) {
    match slot {
        crate::FrameStorageSlotId::Incoming {
            parameter_index,
            abi_stack_byte_offset,
        } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, parameter_index.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, abi_stack_byte_offset.to_le_bytes());
        }
        crate::FrameStorageSlotId::Outgoing(value) => {
            bytes.push(match value.role {
                crate::OutgoingArgumentSlotRole::Argument => 0,
                crate::OutgoingArgumentSlotRole::ValueCopy => 3,
            });
            value.invocation_site.encode_identity(bytes);
            crate::codec_bytes::push_encoded(bytes, value.argument_index.to_le_bytes());
        }
        crate::FrameStorageSlotId::Local(value) => {
            bytes.push(1);
            local_slot(bytes, value);
        }
    }
}
pub(super) fn encode_static(bytes: &mut Vec<u8>, function: &SelectedFunction) {
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
        crate::codec_bytes::push_encoded(bytes, row.byte_size.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, row.alignment.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, row.abi_stack_byte_offset.to_le_bytes());
    }
    encode_len(bytes, function.calls.len());
    for row in &function.calls {
        crate::codec_bytes::push_encoded(bytes, row.instruction.0.to_le_bytes());
        match row.owner {
            target_operations::CallSiteOwner::Operation(operation) => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
            }
            target_operations::CallSiteOwner::ExternalCompletion(edge) => {
                bytes.push(2);

                crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
            }
            target_operations::CallSiteOwner::CleanupAction {
                edge,
                action_ordinal,
            } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, action_ordinal.to_le_bytes());
            }
        }
        blob(
            bytes,
            &row.call
                .canonical_bytes_with_effects(row.effect, &row.ownership),
        );
    }
    encode_len(bytes, function.normalized_foreign_calls.len());
    for row in &function.normalized_foreign_calls {
        crate::codec_bytes::push_encoded(bytes, row.instruction.0.to_le_bytes());
        row.invocation_site.encode_identity(bytes);
        blob(
            bytes,
            &row.call
                .canonical_bytes_with_effects(row.effect, &row.ownership),
        );
    }
    encode_len(bytes, function.memory_accesses.len());
    for row in &function.memory_accesses {
        crate::codec_bytes::push_encoded(bytes, row.instruction.0.to_le_bytes());
        row.origin.encode_identity(bytes);
        crate::codec_bytes::push_encoded(bytes, row.place.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, row.byte_offset.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, row.byte_count.to_le_bytes());
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
                crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
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
                    crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
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
                crate::codec_bytes::push_encoded(bytes, index.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::WriteIndexedPrimitive {
                index,
                value,
                stride,
                extent,
                obligation,
                accepted_fact,
            } => {
                bytes.push(10);
                for identity in [index.get(), value.get(), obligation.get()] {
                    crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
                }
                crate::codec_bytes::push_encoded(bytes, stride.to_le_bytes());
                match extent {
                    Some(extent) => {
                        bytes.push(1);
                        crate::codec_bytes::push_encoded(bytes, extent.to_le_bytes());
                    }
                    None => bytes.push(0),
                }
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::WriteIndexedStructural {
                index,
                stride,
                extent,
                obligation,
                accepted_fact,
            } => {
                bytes.push(13);
                for identity in [index.get(), obligation.get()] {
                    crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
                }
                crate::codec_bytes::push_encoded(bytes, stride.to_le_bytes());
                match extent {
                    Some(extent) => {
                        bytes.push(1);
                        crate::codec_bytes::push_encoded(bytes, extent.to_le_bytes());
                    }
                    None => bytes.push(0),
                }
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::ReadIndexedPrimitive {
                index,
                stride,
                extent,
                obligation,
                accepted_fact,
            } => {
                bytes.push(12);
                for identity in [index.get(), obligation.get()] {
                    crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
                }
                crate::codec_bytes::push_encoded(bytes, stride.to_le_bytes());
                match extent {
                    Some(extent) => {
                        bytes.push(1);
                        crate::codec_bytes::push_encoded(bytes, extent.to_le_bytes());
                    }
                    None => bytes.push(0),
                }
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::ReadElementView {
                index,
                length,
                obligation,
                accepted_fact,
            } => {
                bytes.push(11);
                crate::codec_bytes::push_encoded(bytes, index.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::WriteElementView {
                index,
                length,
                obligation,
                accepted_fact,
            } => {
                bytes.push(14);
                crate::codec_bytes::push_encoded(bytes, index.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
                bytes.extend_from_slice(&accepted_fact.bytes());
            }
            SelectedMemoryAccessRole::ReadReturnedView { producer } => {
                bytes.push(15);
                crate::codec_bytes::push_encoded(bytes, producer.get().to_le_bytes());
            }
            SelectedMemoryAccessRole::ReadMeasuredSpan { length } => {
                bytes.push(16);
                crate::codec_bytes::push_encoded(bytes, length.get().to_le_bytes());
            }
            SelectedMemoryAccessRole::ReadPlace => bytes.push(0),
            SelectedMemoryAccessRole::ReadBacking => bytes.push(18),
            SelectedMemoryAccessRole::WriteBacking => bytes.push(19),
            SelectedMemoryAccessRole::ReadLocal { slot: value } => {
                bytes.push(17);
                local_slot(bytes, value);
            }
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
}

pub(super) fn encode_dynamic(bytes: &mut Vec<u8>, function: &SelectedFunction) {
    encode_len(bytes, function.local_storage_slots.len());
    for row in &function.local_storage_slots {
        encode_local_storage_slot(bytes, row);
    }
    encode_boundary_settlements(bytes, function);
}

pub(super) fn encode_boundary_settlements(bytes: &mut Vec<u8>, function: &SelectedFunction) {
    encode_len(bytes, function.boundary_settlements.len());
    for row in &function.boundary_settlements {
        crate::codec_bytes::push_encoded(bytes, row.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, row.instruction_index.to_le_bytes());
        blob(bytes, &row.settlement.canonical_bytes());
    }
}
