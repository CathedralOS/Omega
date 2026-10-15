//! Whole-value snapshots precede every destination replacement.
//!
//! Stage each input in private activation storage, then copy the staged bytes
//! into the destination after all snapshots and cleanup hooks. Loading every
//! word into a long-lived virtual register made register pressure proportional
//! to aggregate size and forced recovery to spill those words individually.
//! Private slots retain the same parallel-assignment semantics, including swaps
//! and cleanup that changes source storage, with only one chunk live at a time.
//! They are compiler scratch, not new semantic places or ABI destinations.
use super::VirtualRegisterId;
use crate::SelectedInstructionError;
use crate::selection::edge_transfers::{chunks, invalid, stored_transport};
use register_model::ValidatedRegisterConstraintCatalog;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedLocalStorageSlot,
    SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    SelectedSelectionConstraints, SelectedStructuralBinding, VirtualRegister,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{EdgeId, PlaceId};

pub(super) struct Snapshot {
    pointer: VirtualRegisterId,
    byte_size: u32,
    source_byte_offset: u32,
}

pub(super) fn snapshot(
    function_index: usize,
    registers: &mut Vec<VirtualRegister>,
    memory: &mut Vec<SelectedMemoryAccess>,
    slots: &mut Vec<SelectedLocalStorageSlot>,
    instructions: &mut Vec<SelectedInstruction>,
    next_instruction: &mut usize,
    edge: EdgeId,
    bindings: &[SelectedStructuralBinding],
    constraints: &SelectedSelectionConstraints,
    catalog: &ValidatedRegisterConstraintCatalog,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<Vec<Snapshot>, SelectedInstructionError> {
    let mut storage = EdgeStorage {
        function_index,
        registers,
        memory,
        instructions,
        next_instruction,
        edge,
        constraints,
        catalog,
        environment,
    };
    let mut snapshots = Vec::new();
    for (binding_index, binding) in bindings.iter().enumerate() {
        let Some((argument, source_byte_offset, _, byte_size, alignment)) =
            stored_transport(binding.transport)
        else {
            continue;
        };
        let slot = LocalStorageSlotId::EdgeSnapshot {
            edge,
            binding_index: u32::try_from(binding_index).map_err(|_| invalid(function_index))?,
        };
        if slots.iter().any(|existing| existing.id == slot) {
            return Err(invalid(function_index));
        }
        slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: byte_size.max(1),
            alignment,
        });
        let pointer = storage.frame_address(slot, argument, binding.semantic.argument.place)?;
        storage.copy(CopyExtent {
            source: argument,
            source_offset: source_byte_offset,
            destination: pointer,
            byte_size,
            lineage_place: binding.semantic.argument.place,
            lineage_offset: source_byte_offset,
            read_binding: Some(binding),
            write_local: None,
        })?;
        snapshots.push(Snapshot {
            pointer,
            byte_size,
            source_byte_offset,
        });
    }
    Ok(snapshots)
}

pub(super) fn store(
    function_index: usize,
    registers: &mut Vec<VirtualRegister>,
    memory: &mut Vec<SelectedMemoryAccess>,
    instructions: &mut Vec<SelectedInstruction>,
    next_instruction: &mut usize,
    edge: EdgeId,
    bindings: &[SelectedStructuralBinding],
    snapshots: &[Snapshot],
    constraints: &SelectedSelectionConstraints,
    catalog: &ValidatedRegisterConstraintCatalog,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<(), SelectedInstructionError> {
    let mut storage = EdgeStorage {
        function_index,
        registers,
        memory,
        instructions,
        next_instruction,
        edge,
        constraints,
        catalog,
        environment,
    };
    let bindings = bindings
        .iter()
        .filter(|binding| stored_transport(binding.transport).is_some());
    if bindings.clone().count() != snapshots.len() {
        return Err(invalid(function_index));
    }
    for (binding, snapshot) in bindings.zip(snapshots) {
        let Some((argument, _, destination, byte_size, _)) = stored_transport(binding.transport)
        else {
            return Err(invalid(function_index));
        };
        if snapshot.byte_size != byte_size {
            return Err(invalid(function_index));
        }
        let pointer = storage.frame_address(destination, argument, binding.semantic.parameter)?;
        let address_instruction = storage
            .instructions
            .last()
            .ok_or_else(|| invalid(function_index))?
            .id;
        storage.memory.push(SelectedMemoryAccess {
            instruction: address_instruction,
            origin: SelectedMemoryAccessOrigin::Edge(edge),
            place: binding.semantic.parameter,
            byte_offset: 0,
            byte_count: byte_size,
            role: SelectedMemoryAccessRole::AddressLocal { slot: destination },
        });
        storage.copy(CopyExtent {
            source: snapshot.pointer,
            source_offset: 0,
            destination: pointer,
            byte_size,
            lineage_place: binding.semantic.argument.place,
            lineage_offset: snapshot.source_byte_offset,
            read_binding: None,
            write_local: Some((binding.semantic.parameter, destination)),
        })?;
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

struct EdgeStorage<'storage> {
    function_index: usize,
    registers: &'storage mut Vec<VirtualRegister>,
    memory: &'storage mut Vec<SelectedMemoryAccess>,
    instructions: &'storage mut Vec<SelectedInstruction>,
    next_instruction: &'storage mut usize,
    edge: EdgeId,
    constraints: &'storage SelectedSelectionConstraints,
    catalog: &'storage ValidatedRegisterConstraintCatalog,
    environment: &'storage register_environment::ValidatedTargetRegisterEnvironment,
}

impl EdgeStorage<'_> {
    fn instruction_id(&mut self) -> Result<SelectedInstructionId, SelectedInstructionError> {
        let identity =
            u32::try_from(*self.next_instruction).map_err(|_| invalid(self.function_index))?;
        *self.next_instruction = self
            .next_instruction
            .checked_add(1)
            .ok_or_else(|| invalid(self.function_index))?;
        Ok(SelectedInstructionId(identity))
    }

    fn register(
        &mut self,
        input: VirtualRegisterId,
        instruction: SelectedInstructionId,
        place: PlaceId,
        byte_offset: u32,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let source = self
            .registers
            .get(input.0 as usize)
            .ok_or_else(|| invalid(self.function_index))?;
        let output = VirtualRegisterId(
            u32::try_from(self.registers.len()).map_err(|_| invalid(self.function_index))?,
        );
        self.registers.push(VirtualRegister {
            id: output,
            scalar_type: source.scalar_type,
            class: source.class,
            origin: VirtualRegisterOrigin::AbiTransport {
                instruction,
                place,
                byte_offset,
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        Ok(output)
    }

    fn frame_address(
        &mut self,
        slot: LocalStorageSlotId,
        input: VirtualRegisterId,
        place: PlaceId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let instruction = self.instruction_id()?;
        let output = self.register(input, instruction, place, 0)?;
        self.instructions
            .push(super::super::constraints::instruction(
                instruction,
                SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                self.constraints
                    .keys
                    .frame_address
                    .ok_or_else(|| invalid(self.function_index))?,
                &[output],
                provenance(self.edge),
                self.catalog,
            )?);
        Ok(output)
    }

    /// Native displacement limits do not bound the semantic storage extent.
    /// Stage addresses through the target's ordinary address-offset row and
    /// retain the original offsets in provenance and memory footprints.
    fn memory_pointer(
        &mut self,
        pointer: VirtualRegisterId,
        place: PlaceId,
        byte_offset: u32,
        byte_size: u32,
    ) -> Result<(VirtualRegisterId, u32), SelectedInstructionError> {
        if self
            .environment
            .admits_pointer_displacement(byte_offset, byte_size)
        {
            return Ok((pointer, byte_offset));
        }
        let instruction = self.instruction_id()?;
        let output = self.register(pointer, instruction, place, byte_offset)?;
        self.instructions
            .push(super::super::constraints::instruction(
                instruction,
                SelectedInstructionKind::AddressOffset { byte_offset },
                self.constraints
                    .keys
                    .address_offset
                    .ok_or_else(|| invalid(self.function_index))?,
                &[pointer, output],
                provenance(self.edge),
                self.catalog,
            )?);
        Ok((output, 0))
    }

    fn copy(&mut self, extent: CopyExtent) -> Result<(), SelectedInstructionError> {
        for (cursor, width) in chunks(extent.byte_size) {
            let byte_offset = extent
                .source_offset
                .checked_add(cursor)
                .ok_or_else(|| invalid(self.function_index))?;
            let (source, displacement) = self.memory_pointer(
                extent.source,
                extent.lineage_place,
                byte_offset,
                u32::from(width),
            )?;
            let load = self.instruction_id()?;
            let lineage_offset = extent
                .lineage_offset
                .checked_add(cursor)
                .ok_or_else(|| invalid(self.function_index))?;
            let value = self.register(extent.source, load, extent.lineage_place, lineage_offset)?;
            let (kind, key) = match width {
                8 => (
                    SelectedInstructionKind::Load64 {
                        byte_offset: displacement,
                    },
                    self.constraints.keys.load64,
                ),
                4 => (
                    SelectedInstructionKind::Load32 {
                        byte_offset: displacement,
                    },
                    self.constraints.keys.load32,
                ),
                2 => (
                    SelectedInstructionKind::Load16 {
                        byte_offset: displacement,
                    },
                    self.constraints.keys.load16,
                ),
                _ => (
                    SelectedInstructionKind::Load8 {
                        byte_offset: displacement,
                    },
                    self.constraints.keys.load8,
                ),
            };
            self.instructions
                .push(super::super::constraints::instruction(
                    load,
                    kind,
                    key.ok_or_else(|| invalid(self.function_index))?,
                    &[source, value],
                    provenance(self.edge),
                    self.catalog,
                )?);
            if let Some(binding) = extent.read_binding {
                self.memory.extend(
                    binding
                        .source_read_roles()
                        .map(|role| SelectedMemoryAccess {
                            instruction: load,
                            origin: SelectedMemoryAccessOrigin::Edge(self.edge),
                            place: binding.semantic.argument.place,
                            byte_offset,
                            byte_count: u32::from(width),
                            role,
                        }),
                );
            }
            let (destination, displacement) = self.memory_pointer(
                extent.destination,
                extent.lineage_place,
                cursor,
                u32::from(width),
            )?;
            let store = self.instruction_id()?;
            self.instructions
                .push(super::super::constraints::instruction(
                    store,
                    SelectedInstructionKind::Store {
                        byte_offset: displacement,
                        byte_size: width,
                    },
                    self.constraints
                        .keys
                        .store
                        .ok_or_else(|| invalid(self.function_index))?,
                    &[destination, value],
                    provenance(self.edge),
                    self.catalog,
                )?);
            if let Some((place, slot)) = extent.write_local {
                self.memory.push(SelectedMemoryAccess {
                    instruction: store,
                    origin: SelectedMemoryAccessOrigin::Edge(self.edge),
                    place,
                    byte_offset: cursor,
                    byte_count: u32::from(width),
                    role: SelectedMemoryAccessRole::WriteLocal { slot },
                });
            }
        }
        Ok(())
    }
}

pub(super) fn provenance(edge: EdgeId) -> SelectedInstructionProvenance {
    SelectedInstructionProvenance {
        edges: vec![edge],
        ..Default::default()
    }
}
