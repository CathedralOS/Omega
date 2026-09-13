//! Independently check incoming fragments or home loads and exact return registers.
use super::*;
use selected_instructions::{FrameStorageSlotId, SelectedMemoryAccessRole};

pub(super) fn validate(
    source: &LegalizedScalarFunction,
    block: &legalized_operations::LegalizedScalarBlock,
    returned: &legalized_operations::LegalizedScalarReturn,
    replay: &mut Replay<'_>,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let (place, placement, slot) = if let Some((parameter, placement)) =
        crate::selection::aggregate_result_input::returned_parameter(source, &returned.value)
    {
        let place = parameter.semantic.place;
        let slot = selected_instructions::LocalStorageSlotId::StructuralParameter { place };
        let slot = replay
            .transport
            .local_slots
            .iter()
            .any(|home| home.id == slot)
            .then_some(slot);
        (place, placement, slot)
    } else {
        let (slot, placement) =
            crate::selection::aggregate_result_input::returned(source, &returned.value)
                .ok_or_else(invalid)?;
        (
            slot.structural_place().ok_or_else(invalid)?,
            placement,
            Some(slot),
        )
    };
    let empty_return = crate::selection::scalar_call_abi::empty_aggregate_placement(placement);
    let slot = slot.filter(|_| !empty_return);
    if slot.is_some_and(|slot| {
        replay
            .transport
            .local_slots
            .iter()
            .filter(|home| {
                home.id == slot
                    && home.byte_size == u32::from(placement.shape.byte_size)
                    && home.alignment == placement.shape.alignment
            })
            .count()
            != 1
    }) {
        return Err(invalid());
    }
    // Prefer current addressable backing, including owned ABI indirection.
    // Inline inputs with no addressable home retain their captured fragments.
    // Both direct and hidden-pointer results consume this same source choice.
    let pointer = if let Some(slot) = slot {
        let pointer = super::structural_case::temporary(replay, place, 0, false)?;
        super::structural_case::memory(
            replay,
            block.id,
            place,
            0,
            u32::from(placement.shape.byte_size),
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        replay.check_instruction(
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            replay.constraints.keys.frame_address.ok_or_else(invalid)?,
            &[pointer],
            &Default::default(),
        )?;
        Some(pointer)
    } else {
        replay
            .transport
            .pointers
            .iter()
            .find(|(owner, _)| *owner == place)
            .map(|(_, pointer)| *pointer)
    };
    if crate::selection::aggregate_result_input::indirect_result(placement, source.call_plan.policy)
        .is_some()
    {
        super::structural::indirect_results::returned(
            source,
            block,
            returned,
            place,
            pointer.ok_or_else(invalid)?,
            placement,
            replay,
        )?;
        return Ok(());
    }
    let scalar_return = replay.constraints.keys.return_aggregate.is_empty()
        && matches!(
            placement.locations.as_slice(),
            [ValueLocation::Register {
                value_byte_offset: 0,
                byte_size: 8,
                ..
            }]
        )
        && placement.shape == calling_conventions::ValueShape::integer(8, 8);
    let keys = if empty_return {
        std::slice::from_ref(&replay.constraints.keys.return_unit)
    } else if scalar_return {
        std::slice::from_ref(&replay.constraints.keys.return_i64)
    } else {
        &replay.constraints.keys.return_aggregate
    };
    let key = keys.iter().find(|key| {
        row(catalog, **key).is_ok_and(|row| row.operands.len() == placement.locations.len()
            && row.operands.iter().zip(&placement.locations).all(|(operand, location)| {
                matches!(location, ValueLocation::Register { register, .. }
                    if operand.fixed_view.is_some() && operand.fixed_view == environment.fixed_register_view(*register))
            }))
    }).copied().ok_or_else(invalid)?;
    let mut registers = Vec::new();
    if let Some(pointer) = pointer {
        for location in &placement.locations {
            let ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            } = location
            else {
                return Err(invalid());
            };
            let offset = u32::from(*value_byte_offset);
            let register = super::structural_case::temporary(replay, place, offset, false)?;
            super::structural_case::memory(
                replay,
                block.id,
                place,
                offset,
                u32::from(*byte_size),
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            super::aggregate_memory::load(replay, pointer, register, offset, *byte_size)?;
            registers.push(register);
        }
    } else {
        for location in &placement.locations {
            let ValueLocation::Register {
                value_byte_offset, ..
            } = location
            else {
                return Err(invalid());
            };
            let offset = u32::from(*value_byte_offset);
            let input = replay
                .transport
                .fragments
                .iter()
                .find(|(owner, position, _)| *owner == place && *position == offset)
                .map(|(_, _, register)| *register)
                .ok_or_else(invalid)?;
            let output = super::structural::result(replay, place, offset)?;
            replay.check_instruction(
                SelectedInstructionKind::CopyI64,
                replay.constraints.keys.copy_i64,
                &[input, output],
                &Default::default(),
            )?;
            registers.push(output);
        }
    }
    let provenance = replay.settle_provenance(SelectedInstructionProvenance {
        edges: vec![returned.edge],
        fuel: returned.fuel.clone(),
        ..Default::default()
    });
    let SelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &replay.block.terminator
    else {
        return Err(invalid());
    };
    if *psi_return_edge != returned.edge
        || instruction.id.0 as usize != replay.instruction_cursor
        || instruction.kind
            != (if empty_return {
                SelectedInstructionKind::ReturnUnit
            } else if scalar_return {
                SelectedInstructionKind::ReturnScalar
            } else {
                SelectedInstructionKind::ReturnAggregate {
                    fragment_count: registers.len() as u8,
                }
            })
        || instruction.constraint != key
        || instruction
            .operands
            .iter()
            .map(|operand| operand.virtual_register)
            .ne(registers)
        || instruction.provenance != provenance
    {
        return Err(invalid());
    }
    Ok(())
}
