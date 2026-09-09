//! The selected edge loads payloads before committing destination bindings.
use super::*;
use selected_instructions::{
    FrameStorageSlotId, SelectedCasePayloadTransport, SelectedLocalStorageSlot,
    SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
#[cfg(test)]
mod tests;

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare(
    function: usize,
    successor: &mut SelectedSuccessor,
    block_position: usize,
    registers: &mut Vec<VirtualRegister>,
    memory: &mut Vec<SelectedMemoryAccess>,
    slots: &[SelectedLocalStorageSlot],
    next_instruction: &mut usize,
    constraints: &SelectedSelectionConstraints,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<Option<SelectedBlock>, SelectedInstructionError> {
    let invalid = || super::super::invalid(function);
    let case = successor.structural_case.as_ref().ok_or_else(invalid)?;
    if !successor.bindings.is_empty() || !successor.structural_bindings.is_empty() {
        return Err(invalid());
    }
    if case
        .payloads
        .iter()
        .all(|payload| payload.transport == SelectedCasePayloadTransport::Unused)
    {
        return Ok(None);
    }
    let slot = slots
        .iter()
        .find(|slot| slot.id == case.slot)
        .ok_or_else(invalid)?;
    let place = slot.id.structural_place().ok_or_else(invalid)?;
    let bridge_id = SelectedBlockId(block_position.try_into().map_err(|_| invalid())?);
    let mut continuation = successor.clone();
    continuation.role = SelectedSuccessorRole::EdgeTransferContinuation;
    continuation.fuel.clear();
    let continued = continuation.structural_case.as_mut().ok_or_else(invalid)?;
    continued.trivial_affine_discards.clear();
    let mut instructions = Vec::new();
    for payload in &mut continued.payloads {
        let parameter = match payload.transport {
            SelectedCasePayloadTransport::Unused => continue,
            SelectedCasePayloadTransport::Unmaterialized { parameter } => parameter,
            SelectedCasePayloadTransport::Registers { .. } => return Err(invalid()),
        };
        let destination = registers
            .get(parameter.0 as usize)
            .cloned()
            .ok_or_else(invalid)?;
        let ScalarType::Integer(integer) = payload.semantic.parameter.scalar_type else {
            return Err(invalid());
        };
        let (load_kind, load_key, byte_count) = match integer.bits() {
            32 => (
                SelectedInstructionKind::Load32 {
                    byte_offset: payload.semantic.field_byte_offset,
                },
                constraints.keys.load32,
                4,
            ),
            64 => (
                SelectedInstructionKind::Load64 {
                    byte_offset: payload.semantic.field_byte_offset,
                },
                constraints.keys.load64,
                8,
            ),
            _ => return Err(invalid()),
        };
        if destination.scalar_type != payload.semantic.parameter.scalar_type
            || destination.definition_site != Some(payload.semantic.parameter.definition_site)
            || payload
                .semantic
                .field_byte_offset
                .checked_add(byte_count)
                .is_none_or(|end| end > slot.byte_size)
            || payload.semantic.field_byte_offset % byte_count != 0
        {
            return Err(invalid());
        }
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
        let provenance = || SelectedInstructionProvenance {
            edges: vec![successor.psi_edge],
            ..Default::default()
        };
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
            origin: SelectedMemoryAccessOrigin::Edge(successor.psi_edge),
            place,
            byte_offset: 0,
            byte_count: slot.byte_size,
            role: SelectedMemoryAccessRole::AddressLocal { slot: slot.id },
        });
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
            origin: SelectedMemoryAccessOrigin::Edge(successor.psi_edge),
            place,
            byte_offset: payload.semantic.field_byte_offset,
            byte_count,
            role: SelectedMemoryAccessRole::ReadPlace,
        });
        payload.transport = SelectedCasePayloadTransport::Registers {
            argument: loaded,
            parameter,
        };
    }
    let jump_id = SelectedInstructionId((*next_instruction).try_into().map_err(|_| invalid())?);
    *next_instruction += 1;
    let jump = crate::selection::constraints::instruction(
        jump_id,
        SelectedInstructionKind::Jump,
        constraints.keys.jump,
        &[],
        Default::default(),
        catalog,
    )?;
    let bridge = SelectedBlock {
        id: bridge_id,
        origin: SelectedBlockOrigin::EdgeTransfer {
            edge: successor.psi_edge,
            target: successor.source_target,
        },
        instructions,
        terminator: SelectedTerminator::Jump {
            instruction: jump,
            successor: continuation,
        },
    };
    successor.block = bridge_id;
    for payload in &mut successor
        .structural_case
        .as_mut()
        .ok_or_else(invalid)?
        .payloads
    {
        payload.transport = SelectedCasePayloadTransport::Unused;
    }
    Ok(Some(bridge))
}
