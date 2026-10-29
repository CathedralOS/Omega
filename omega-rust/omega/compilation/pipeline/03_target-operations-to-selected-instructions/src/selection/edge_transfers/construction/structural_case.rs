//! Capture selected payloads into the common edge snapshot frontier.
use super::{
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
    SelectedSelectionConstraints, ValidatedRegisterConstraintCatalog, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use crate::SelectedInstructionError;
use crate::selection::scalar_call_abi::load_transfer;
use selected_instructions::SelectedSuccessor;
use selected_instructions::{
    FrameStorageSlotId, SelectedCasePayloadTransport, SelectedLocalStorageSlot,
    SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};

#[allow(clippy::too_many_arguments)]
pub(super) fn snapshot(
    function: usize,
    successor: &mut SelectedSuccessor,
    registers: &mut Vec<VirtualRegister>,
    memory: &mut Vec<SelectedMemoryAccess>,
    slots: &[SelectedLocalStorageSlot],
    next_instruction: &mut usize,
    constraints: &SelectedSelectionConstraints,
    catalog: &ValidatedRegisterConstraintCatalog,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    instructions: &mut Vec<selected_instructions::SelectedInstruction>,
) -> Result<(), SelectedInstructionError> {
    let invalid = || super::super::invalid(function);
    let psi_edge = successor.psi_edge.ok_or_else(invalid)?;
    let case = successor.structural_case.as_mut().ok_or_else(invalid)?;
    if !successor.bindings.is_empty() {
        return Err(invalid());
    }
    let (place, byte_bound) = match case.source {
        selected_instructions::SelectedCaseDispatchSource::Local { slot } => {
            let slot = slots
                .iter()
                .find(|row| row.id == slot)
                .ok_or_else(invalid)?;
            (
                slot.id.structural_place().ok_or_else(invalid)?,
                slot.byte_size,
            )
        }
        selected_instructions::SelectedCaseDispatchSource::Borrowed {
            place, byte_size, ..
        } => (place, byte_size),
    };
    let source = case.source;
    for payload in &mut case.payloads {
        let parameter = match payload.transport {
            SelectedCasePayloadTransport::Unused => continue,
            SelectedCasePayloadTransport::Unmaterialized { parameter } => parameter,
            SelectedCasePayloadTransport::Registers { .. } => return Err(invalid()),
        };
        let destination = registers
            .get(parameter.0 as usize)
            .cloned()
            .ok_or_else(invalid)?;
        let shape =
            crate::selection::scalar_call_abi::scalar_shape(payload.semantic.parameter.scalar_type)
                .ok_or_else(invalid)?;
        let (load_key, byte_count) = match shape.byte_size {
            1 => (constraints.keys.load8, 1),
            2 => (constraints.keys.load16, 2),
            4 => (constraints.keys.load32, 4),
            8 => (constraints.keys.load64, 8),
            _ => return Err(invalid()),
        };
        if destination.scalar_type != payload.semantic.parameter.scalar_type
            || destination.definition_site != Some(payload.semantic.parameter.definition_site)
            || payload
                .semantic
                .field_byte_offset
                .checked_add(byte_count)
                .is_none_or(|end| end > byte_bound)
            || payload.semantic.field_byte_offset % byte_count != 0
        {
            return Err(invalid());
        }
        let provenance = || SelectedInstructionProvenance {
            edges: vec![psi_edge],
            ..Default::default()
        };
        let pointer = match source {
            selected_instructions::SelectedCaseDispatchSource::Local { slot } => {
                let slot = slots
                    .iter()
                    .find(|row| row.id == slot)
                    .ok_or_else(invalid)?;
                let address_instruction =
                    SelectedInstructionId((*next_instruction).try_into().map_err(|_| invalid())?);
                *next_instruction += 1;
                let pointer = VirtualRegisterId(registers.len().try_into().map_err(|_| invalid())?);
                registers.push(VirtualRegister {
                    id: pointer,
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
                    ),
                    class: destination.class,
                    origin: VirtualRegisterOrigin::AbiTransport {
                        instruction: address_instruction,
                        place,
                        byte_offset: 0,
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                });
                instructions.push(crate::selection::constraints::instruction(
                    address_instruction,
                    SelectedInstructionKind::FrameAddress {
                        slot: FrameStorageSlotId::Local(slot.id),
                        byte_offset: 0,
                    },
                    constraints.keys.frame_address.ok_or_else(invalid)?,
                    &[pointer],
                    provenance(),
                    catalog,
                )?);
                memory.push(SelectedMemoryAccess {
                    instruction: address_instruction,
                    origin: SelectedMemoryAccessOrigin::Edge(psi_edge),
                    place,
                    byte_offset: 0,
                    byte_count: slot.byte_size,
                    role: SelectedMemoryAccessRole::AddressLocal { slot: slot.id },
                });
                pointer
            }
            // The referent pointer was retained at entry; the bridge needs no
            // addressing instruction of its own.
            selected_instructions::SelectedCaseDispatchSource::Borrowed { pointer, .. } => {
                if registers.get(pointer.0 as usize).is_none_or(|register| {
                    register.scalar_type
                        != ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 pointer type"),
                        )
                }) {
                    return Err(invalid());
                }
                pointer
            }
        };
        // Preserve the root-relative semantic footprint while staging an
        // address only when this target cannot encode the load displacement.
        let (pointer, displacement) = if environment
            .admits_pointer_displacement(payload.semantic.field_byte_offset, byte_count)
        {
            (pointer, payload.semantic.field_byte_offset)
        } else {
            let input = registers
                .get(pointer.0 as usize)
                .cloned()
                .ok_or_else(invalid)?;
            let address_instruction =
                SelectedInstructionId((*next_instruction).try_into().map_err(|_| invalid())?);
            *next_instruction += 1;
            let address = VirtualRegisterId(registers.len().try_into().map_err(|_| invalid())?);
            registers.push(VirtualRegister {
                id: address,
                scalar_type: input.scalar_type,
                class: input.class,
                origin: VirtualRegisterOrigin::AbiTransport {
                    instruction: address_instruction,
                    place,
                    byte_offset: payload.semantic.field_byte_offset,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
            instructions.push(crate::selection::constraints::instruction(
                address_instruction,
                SelectedInstructionKind::AddressOffset {
                    byte_offset: payload.semantic.field_byte_offset,
                },
                constraints.keys.address_offset.ok_or_else(invalid)?,
                &[pointer, address],
                provenance(),
                catalog,
            )?);
            (address, 0)
        };
        let (load_kind, _) =
            load_transfer(&constraints.keys, byte_count, displacement).ok_or_else(invalid)?;
        let load_instruction =
            SelectedInstructionId((*next_instruction).try_into().map_err(|_| invalid())?);
        *next_instruction += 1;
        let loaded = VirtualRegisterId(registers.len().try_into().map_err(|_| invalid())?);
        registers.push(VirtualRegister {
            id: loaded,
            scalar_type: destination.scalar_type,
            class: destination.class,
            origin: VirtualRegisterOrigin::StructuralObservation {
                instruction: load_instruction,
                place,
                byte_offset: payload.semantic.field_byte_offset,
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        instructions.push(crate::selection::constraints::instruction(
            load_instruction,
            load_kind,
            load_key.ok_or_else(invalid)?,
            &[pointer, loaded],
            provenance(),
            catalog,
        )?);
        memory.push(SelectedMemoryAccess {
            instruction: load_instruction,
            origin: SelectedMemoryAccessOrigin::Edge(psi_edge),
            place,
            byte_offset: payload.semantic.field_byte_offset,
            byte_count,
            role: SelectedMemoryAccessRole::ReadPlace,
        });
        // Exact-width loads leave zero-extended bits. A state arrival must
        // restore the signed carrier before its first consumer, just as an
        // incoming machine argument does. Only the load owns a memory read.
        let loaded = if matches!(destination.scalar_type, ScalarType::Integer(integer)
            if integer.sign() == IntegerSign::Signed && matches!(integer.bits(), 8 | 16 | 32))
        {
            let normalization =
                SelectedInstructionId((*next_instruction).try_into().map_err(|_| invalid())?);
            *next_instruction += 1;
            let normalized = VirtualRegisterId(registers.len().try_into().map_err(|_| invalid())?);
            registers.push(VirtualRegister {
                id: normalized,
                scalar_type: destination.scalar_type,
                class: destination.class,
                origin: VirtualRegisterOrigin::AbiTransport {
                    instruction: normalization,
                    place,
                    byte_offset: payload.semantic.field_byte_offset,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
            instructions.push(crate::selection::constraints::instruction(
                normalization,
                crate::selection::scalar_call_abi::integer_carrier_normalization(
                    destination.scalar_type,
                ),
                constraints.keys.copy_i64,
                &[loaded, normalized],
                provenance(),
                catalog,
            )?);
            normalized
        } else {
            loaded
        };
        payload.transport = SelectedCasePayloadTransport::Registers {
            argument: loaded,
            parameter,
        };
    }
    Ok(())
}
