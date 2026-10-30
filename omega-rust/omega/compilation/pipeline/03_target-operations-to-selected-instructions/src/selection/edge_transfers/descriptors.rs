//! Whole-value snapshots precede every destination replacement.
//!
//! Stage each input in private activation storage, then copy the staged bytes
//! into the destination after all snapshots and cleanup hooks. Loading every
//! word into a long-lived virtual register made register pressure proportional
//! to aggregate size and forced recovery to spill those words individually.
//! Private slots retain the same parallel-assignment semantics, including swaps
//! and cleanup that changes source storage, with only one chunk live at a time.
//! They are compiler scratch, not new semantic places or ABI destinations.
use super::emission::{Emission, provenance};
use crate::SelectedInstructionError;
use crate::selection::edge_transfers::{chunks, invalid, stored_transport};
use crate::selection::scalar_call_abi::load_transfer;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedInstructionKind, SelectedLocalStorageSlot,
    SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    SelectedStructuralBinding, VirtualRegisterId,
};
use semantic_vocabulary::PlaceId;

pub(super) struct Snapshot {
    pointer: VirtualRegisterId,
    byte_size: u32,
    source_byte_offset: u32,
}

pub(super) fn snapshot(
    emission: &mut Emission<'_>,
    slots: &mut Vec<SelectedLocalStorageSlot>,
    bindings: &[SelectedStructuralBinding],
) -> Result<Vec<Snapshot>, SelectedInstructionError> {
    let function = emission.function;
    let mut snapshots = Vec::new();
    for (binding_index, binding) in bindings.iter().enumerate() {
        let Some((argument, source_byte_offset, _, byte_size, alignment)) =
            stored_transport(binding.transport)
        else {
            continue;
        };
        let slot = LocalStorageSlotId::EdgeSnapshot {
            edge: emission.edge,
            binding_index: u32::try_from(binding_index).map_err(|_| invalid(function))?,
        };
        if slots.iter().any(|existing| existing.id == slot) {
            return Err(invalid(function));
        }
        slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: byte_size.max(1),
            alignment,
        });
        let pointer = frame_address(emission, slot, binding.semantic.argument.place)?;
        copy(
            emission,
            CopyExtent {
                source: argument,
                source_offset: source_byte_offset,
                destination: pointer,
                byte_size,
                lineage_place: binding.semantic.argument.place,
                lineage_offset: source_byte_offset,
                read_binding: Some(binding),
                write_local: None,
            },
        )?;
        snapshots.push(Snapshot {
            pointer,
            byte_size,
            source_byte_offset,
        });
    }
    Ok(snapshots)
}

pub(super) fn store(
    emission: &mut Emission<'_>,
    bindings: &[SelectedStructuralBinding],
    snapshots: &[Snapshot],
) -> Result<(), SelectedInstructionError> {
    let function = emission.function;
    let bindings = bindings
        .iter()
        .filter(|binding| stored_transport(binding.transport).is_some());
    if bindings.clone().count() != snapshots.len() {
        return Err(invalid(function));
    }
    for (binding, snapshot) in bindings.zip(snapshots) {
        let Some((_, _, destination, byte_size, _)) = stored_transport(binding.transport) else {
            return Err(invalid(function));
        };
        if snapshot.byte_size != byte_size {
            return Err(invalid(function));
        }
        let pointer = frame_address(emission, destination, binding.semantic.parameter)?;
        let address_instruction = emission
            .instructions
            .last()
            .ok_or_else(|| invalid(function))?
            .id;
        emission.memory.push(SelectedMemoryAccess {
            instruction: address_instruction,
            origin: SelectedMemoryAccessOrigin::Edge(emission.edge),
            place: binding.semantic.parameter,
            byte_offset: 0,
            byte_count: byte_size,
            role: SelectedMemoryAccessRole::AddressLocal { slot: destination },
        });
        copy(
            emission,
            CopyExtent {
                source: snapshot.pointer,
                source_offset: 0,
                destination: pointer,
                byte_size,
                lineage_place: binding.semantic.argument.place,
                lineage_offset: snapshot.source_byte_offset,
                read_binding: None,
                write_local: Some((binding.semantic.parameter, destination)),
            },
        )?;
    }
    Ok(())
}

/// A physical copy carries only the semantic footprints it actually realizes.
/// Snapshot writes and reads are private frame accesses, like allocator scratch;
/// assigning them the input place would invent a semantic write or let a later
/// source write appear to change the frozen snapshot.
struct CopyExtent<'binding> {
    source: VirtualRegisterId,
    source_offset: u32,
    destination: VirtualRegisterId,
    byte_size: u32,
    lineage_place: PlaceId,
    lineage_offset: u32,
    read_binding: Option<&'binding SelectedStructuralBinding>,
    write_local: Option<(PlaceId, LocalStorageSlotId)>,
}

/// The address of one storage extent: no operand, one declared word.
fn frame_address(
    emission: &mut Emission<'_>,
    slot: LocalStorageSlotId,
    place: PlaceId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let function = emission.function;
    let key = emission
        .constraints
        .keys
        .frame_address
        .ok_or_else(|| invalid(function))?;
    emission.emit(
        SelectedInstructionKind::FrameAddress {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset: 0,
        },
        key,
        &[],
        place,
        0,
        None,
    )
}

/// Native displacement limits do not bound the semantic storage extent.
/// Stage addresses through the target's ordinary address-offset row and
/// retain the original offsets in provenance and memory footprints.
fn memory_pointer(
    emission: &mut Emission<'_>,
    pointer: VirtualRegisterId,
    place: PlaceId,
    byte_offset: u32,
    byte_size: u32,
) -> Result<(VirtualRegisterId, u32), SelectedInstructionError> {
    let function = emission.function;
    if emission
        .environment
        .admits_pointer_displacement(byte_offset, byte_size)
    {
        return Ok((pointer, byte_offset));
    }
    let key = emission
        .constraints
        .keys
        .address_offset
        .ok_or_else(|| invalid(function))?;
    let output = emission.emit(
        SelectedInstructionKind::AddressOffset { byte_offset },
        key,
        &[pointer],
        place,
        byte_offset,
        None,
    )?;
    Ok((output, 0))
}

fn copy(emission: &mut Emission<'_>, extent: CopyExtent) -> Result<(), SelectedInstructionError> {
    let function = emission.function;
    for (cursor, width) in chunks(extent.byte_size) {
        let byte_offset = extent
            .source_offset
            .checked_add(cursor)
            .ok_or_else(|| invalid(function))?;
        let (source, displacement) = memory_pointer(
            emission,
            extent.source,
            extent.lineage_place,
            byte_offset,
            u32::from(width),
        )?;
        let lineage_offset = extent
            .lineage_offset
            .checked_add(cursor)
            .ok_or_else(|| invalid(function))?;
        let (kind, key) = load_transfer(&emission.constraints.keys, u32::from(width), displacement)
            .ok_or_else(|| invalid(function))?;
        let value = emission.emit(
            kind,
            key.ok_or_else(|| invalid(function))?,
            &[source],
            extent.lineage_place,
            lineage_offset,
            None,
        )?;
        if let Some(binding) = extent.read_binding {
            let load = emission
                .instructions
                .last()
                .ok_or_else(|| invalid(function))?
                .id;
            emission.memory.extend(
                binding
                    .source_read_roles()
                    .map(|role| SelectedMemoryAccess {
                        instruction: load,
                        origin: SelectedMemoryAccessOrigin::Edge(emission.edge),
                        place: binding.semantic.argument.place,
                        byte_offset,
                        byte_count: u32::from(width),
                        role,
                    }),
            );
        }
        let (destination, displacement) = memory_pointer(
            emission,
            extent.destination,
            extent.lineage_place,
            cursor,
            u32::from(width),
        )?;
        let store = emission.instruction_id()?;
        emission
            .instructions
            .push(super::super::constraints::instruction(
                store,
                SelectedInstructionKind::Store {
                    byte_offset: displacement,
                    byte_size: width,
                },
                emission
                    .constraints
                    .keys
                    .store
                    .ok_or_else(|| invalid(function))?,
                &[destination, value],
                provenance(emission.edge),
                emission.catalog,
            )?);
        if let Some((place, slot)) = extent.write_local {
            emission.memory.push(SelectedMemoryAccess {
                instruction: store,
                origin: SelectedMemoryAccessOrigin::Edge(emission.edge),
                place,
                byte_offset: cursor,
                byte_count: u32::from(width),
                role: SelectedMemoryAccessRole::WriteLocal { slot },
            });
        }
    }
    Ok(())
}
