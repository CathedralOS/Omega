//! Independently reconstruct exact edge storage reads before destination writes.
use super::*;
use selected_instructions::{
    FrameStorageSlotId, SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    SelectedStructuralTransport,
};

pub(super) fn check(
    function: usize,
    prepared: &SelectedFunction,
    bridge: &SelectedBlock,
    continuation: &SelectedSuccessor,
    scalar_count: usize,
    instruction_start: usize,
    register_start: usize,
    original_register_count: usize,
    constraints: &SelectedSelectionConstraints,
) -> Result<Vec<SelectedMemoryAccess>, SelectedInstructionError> {
    let error = || invalid(function);
    let total_chunks = continuation
        .structural_bindings
        .iter()
        .filter_map(|binding| stored_transport(binding.transport))
        .map(|(_, _, bytes, _)| chunks(bytes).len())
        .sum::<usize>();
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    let mut chunk_index = 0;
    let mut store_index = scalar_count * 2 + total_chunks;
    let mut address_index = 0;
    for binding in &continuation.structural_bindings {
        let Some((argument, destination, byte_size, whole)) = stored_transport(binding.transport)
        else {
            continue;
        };
        if argument.0 as usize >= original_register_count
            || byte_size == 0
            || destination
                != (selected_instructions::LocalStorageSlotId::StructuralBlockParameter {
                    block: continuation.source_target,
                    place: binding.semantic.parameter,
                })
        {
            return Err(error());
        }
        if let SelectedStructuralTransport::WholeValue {
            byte_size,
            alignment,
            ..
        } = binding.transport
            && prepared
                .local_storage_slots
                .iter()
                .filter(|slot| {
                    slot.id == destination
                        && slot.byte_size == u32::from(byte_size)
                        && slot.alignment == alignment
                })
                .count()
                != 1
        {
            return Err(error());
        }
        let original = prepared
            .virtual_registers
            .get(argument.0 as usize)
            .ok_or_else(error)?;
        let provenance = SelectedInstructionProvenance {
            edges: vec![continuation.psi_edge],
            ..Default::default()
        };
        let destination_pointer = if whole {
            let address = bridge.instructions.get(store_index).ok_or_else(error)?;
            let index = register_start + scalar_count * 2 + total_chunks + address_index;
            let register = prepared.virtual_registers.get(index).ok_or_else(error)?;
            if address.id.0 as usize != instruction_start + store_index
                || address.kind
                    != (SelectedInstructionKind::FrameAddress {
                        slot: FrameStorageSlotId::Local(destination),
                        byte_offset: 0,
                    })
                || Some(address.constraint) != constraints.keys.frame_address
                || address.provenance != provenance
                || address.operands.len() != 1
                || address.operands[0].virtual_register != register.id
                || !transport_register_matches(
                    register,
                    index,
                    original,
                    address.id,
                    binding.semantic.parameter,
                    0,
                )
            {
                return Err(error());
            }
            writes.push(SelectedMemoryAccess {
                instruction: address.id,
                origin: SelectedMemoryAccessOrigin::Edge(continuation.psi_edge),
                place: binding.semantic.parameter,
                byte_offset: 0,
                byte_count: byte_size,
                role: SelectedMemoryAccessRole::AddressLocal { slot: destination },
            });
            store_index += 1;
            address_index += 1;
            Some(register.id)
        } else {
            None
        };
        for (offset, width) in chunks(byte_size) {
            let load_position = scalar_count + chunk_index;
            let load = bridge.instructions.get(load_position).ok_or_else(error)?;
            let store = bridge.instructions.get(store_index).ok_or_else(error)?;
            let index = register_start + load_position;
            let register = prepared.virtual_registers.get(index).ok_or_else(error)?;
            let (kind, key) = match width {
                8 => (
                    SelectedInstructionKind::Load64 {
                        byte_offset: offset,
                    },
                    constraints.keys.load64,
                ),
                4 => (
                    SelectedInstructionKind::Load32 {
                        byte_offset: offset,
                    },
                    constraints.keys.load32,
                ),
                2 => (
                    SelectedInstructionKind::Load16 {
                        byte_offset: offset,
                    },
                    constraints.keys.load16,
                ),
                _ => (
                    SelectedInstructionKind::Load8 {
                        byte_offset: offset,
                    },
                    constraints.keys.load8,
                ),
            };
            if load.id.0 as usize != instruction_start + load_position
                || load.kind != kind
                || Some(load.constraint) != key
                || load.provenance != provenance
                || load.operands.len() != 2
                || load.operands[0].virtual_register != argument
                || load.operands[1].virtual_register != register.id
                || !transport_register_matches(
                    register,
                    index,
                    original,
                    load.id,
                    binding.semantic.argument.place,
                    offset,
                )
            {
                return Err(error());
            }
            let (kind, key, operands) = if let Some(pointer) = destination_pointer {
                (
                    SelectedInstructionKind::Store {
                        byte_offset: offset,
                        byte_size: width,
                    },
                    constraints.keys.store,
                    vec![pointer, register.id],
                )
            } else {
                (
                    SelectedInstructionKind::Store64 {
                        slot: FrameStorageSlotId::Local(destination),
                        byte_offset: offset,
                    },
                    constraints.keys.store64,
                    vec![register.id],
                )
            };
            if store.id.0 as usize != instruction_start + store_index
                || store.kind != kind
                || Some(store.constraint) != key
                || store.provenance != provenance
                || store
                    .operands
                    .iter()
                    .map(|operand| operand.virtual_register)
                    .ne(operands)
            {
                return Err(error());
            }
            reads.push(SelectedMemoryAccess {
                instruction: load.id,
                origin: SelectedMemoryAccessOrigin::Edge(continuation.psi_edge),
                place: binding.semantic.argument.place,
                byte_offset: offset,
                byte_count: u32::from(width),
                role: SelectedMemoryAccessRole::ReadPlace,
            });
            writes.push(SelectedMemoryAccess {
                instruction: store.id,
                origin: SelectedMemoryAccessOrigin::Edge(continuation.psi_edge),
                place: binding.semantic.parameter,
                byte_offset: offset,
                byte_count: u32::from(width),
                role: SelectedMemoryAccessRole::WriteLocal { slot: destination },
            });
            chunk_index += 1;
            store_index += 1;
        }
    }
    if store_index != bridge.instructions.len() {
        return Err(error());
    }
    reads.extend(writes);
    Ok(reads)
}

fn transport_register_matches(
    register: &VirtualRegister,
    index: usize,
    original: &VirtualRegister,
    instruction: SelectedInstructionId,
    place: semantic_vocabulary::PlaceId,
    byte_offset: u32,
) -> bool {
    register.id.0 as usize == index
        && register.scalar_type == original.scalar_type
        && register.class == original.class
        && register.definition_site.is_none()
        && register.entry_fixed_view.is_none()
        && register.origin
            == (VirtualRegisterOrigin::AbiTransport {
                instruction,
                place,
                byte_offset,
            })
}
