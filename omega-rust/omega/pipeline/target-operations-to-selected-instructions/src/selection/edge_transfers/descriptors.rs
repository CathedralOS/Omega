//! Descriptor and whole-value snapshots precede every destination replacement.
use super::*;
use selected_instructions::{
    FrameStorageSlotId, SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    SelectedStructuralBinding,
};

pub(super) fn snapshot(
    function_index: usize,
    registers: &mut Vec<VirtualRegister>,
    memory: &mut Vec<SelectedMemoryAccess>,
    instructions: &mut Vec<SelectedInstruction>,
    next_instruction: &mut usize,
    edge: semantic_vocabulary::EdgeId,
    bindings: &[SelectedStructuralBinding],
    constraints: &SelectedSelectionConstraints,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<Vec<Vec<VirtualRegisterId>>, SelectedInstructionError> {
    let mut snapshots = Vec::new();
    for binding in bindings {
        let Some((argument, _, byte_size, _)) = stored_transport(binding.transport) else {
            continue;
        };
        let input = registers
            .get(argument.0 as usize)
            .cloned()
            .ok_or_else(|| invalid(function_index))?;
        let mut words = Vec::new();
        for (byte_offset, width) in chunks(byte_size) {
            let id = SelectedInstructionId(
                (*next_instruction)
                    .try_into()
                    .map_err(|_| invalid(function_index))?,
            );
            *next_instruction += 1;
            let output = VirtualRegisterId(
                registers
                    .len()
                    .try_into()
                    .map_err(|_| invalid(function_index))?,
            );
            registers.push(VirtualRegister {
                id: output,
                scalar_type: input.scalar_type,
                class: input.class,
                origin: VirtualRegisterOrigin::AbiTransport {
                    instruction: id,
                    place: binding.semantic.argument.place,
                    byte_offset,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
            instructions.push(super::super::constraints::instruction(
                id,
                match width {
                    8 => SelectedInstructionKind::Load64 { byte_offset },
                    4 => SelectedInstructionKind::Load32 { byte_offset },
                    2 => SelectedInstructionKind::Load16 { byte_offset },
                    _ => SelectedInstructionKind::Load8 { byte_offset },
                },
                match width {
                    8 => constraints.keys.load64,
                    4 => constraints.keys.load32,
                    2 => constraints.keys.load16,
                    _ => constraints.keys.load8,
                }
                .ok_or_else(|| invalid(function_index))?,
                &[argument, output],
                provenance(edge),
                catalog,
            )?);
            memory.push(SelectedMemoryAccess {
                instruction: id,
                origin: SelectedMemoryAccessOrigin::Edge(edge),
                place: binding.semantic.argument.place,
                byte_offset,
                byte_count: u32::from(width),
                role: SelectedMemoryAccessRole::ReadPlace,
            });
            words.push(output);
        }
        snapshots.push(words);
    }
    Ok(snapshots)
}

pub(super) fn store(
    function_index: usize,
    registers: &mut Vec<VirtualRegister>,
    memory: &mut Vec<SelectedMemoryAccess>,
    instructions: &mut Vec<SelectedInstruction>,
    next_instruction: &mut usize,
    edge: semantic_vocabulary::EdgeId,
    bindings: &[SelectedStructuralBinding],
    snapshots: &[Vec<VirtualRegisterId>],
    constraints: &SelectedSelectionConstraints,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    for (binding, words) in bindings
        .iter()
        .filter(|binding| stored_transport(binding.transport).is_some())
        .zip(snapshots)
    {
        let Some((argument, destination, byte_size, whole)) = stored_transport(binding.transport)
        else {
            return Err(invalid(function_index));
        };
        let pointer = if whole {
            let id = SelectedInstructionId(
                (*next_instruction)
                    .try_into()
                    .map_err(|_| invalid(function_index))?,
            );
            *next_instruction += 1;
            let output = VirtualRegisterId(
                registers
                    .len()
                    .try_into()
                    .map_err(|_| invalid(function_index))?,
            );
            let source = registers
                .get(argument.0 as usize)
                .ok_or_else(|| invalid(function_index))?
                .clone();
            registers.push(VirtualRegister {
                id: output,
                scalar_type: source.scalar_type,
                class: source.class,
                origin: VirtualRegisterOrigin::AbiTransport {
                    instruction: id,
                    place: binding.semantic.parameter,
                    byte_offset: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
            instructions.push(super::super::constraints::instruction(
                id,
                SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(destination),
                    byte_offset: 0,
                },
                constraints
                    .keys
                    .frame_address
                    .ok_or_else(|| invalid(function_index))?,
                &[output],
                provenance(edge),
                catalog,
            )?);
            memory.push(SelectedMemoryAccess {
                instruction: id,
                origin: SelectedMemoryAccessOrigin::Edge(edge),
                place: binding.semantic.parameter,
                byte_offset: 0,
                byte_count: byte_size,
                role: SelectedMemoryAccessRole::AddressLocal { slot: destination },
            });
            Some(output)
        } else {
            None
        };
        for ((byte_offset, width), input) in chunks(byte_size).into_iter().zip(words) {
            let id = SelectedInstructionId(
                (*next_instruction)
                    .try_into()
                    .map_err(|_| invalid(function_index))?,
            );
            *next_instruction += 1;
            let (kind, key, operands) = if let Some(pointer) = pointer {
                (
                    SelectedInstructionKind::Store {
                        byte_offset,
                        byte_size: width,
                    },
                    constraints.keys.store,
                    vec![pointer, *input],
                )
            } else {
                (
                    SelectedInstructionKind::Store64 {
                        slot: FrameStorageSlotId::Local(destination),
                        byte_offset,
                    },
                    constraints.keys.store64,
                    vec![*input],
                )
            };
            instructions.push(super::super::constraints::instruction(
                id,
                kind,
                key.ok_or_else(|| invalid(function_index))?,
                &operands,
                provenance(edge),
                catalog,
            )?);
            memory.push(SelectedMemoryAccess {
                instruction: id,
                origin: SelectedMemoryAccessOrigin::Edge(edge),
                place: binding.semantic.parameter,
                byte_offset,
                byte_count: u32::from(width),
                role: SelectedMemoryAccessRole::WriteLocal { slot: destination },
            });
        }
    }
    Ok(())
}

pub(super) fn provenance(edge: semantic_vocabulary::EdgeId) -> SelectedInstructionProvenance {
    SelectedInstructionProvenance {
        edges: vec![edge],
        ..Default::default()
    }
}
