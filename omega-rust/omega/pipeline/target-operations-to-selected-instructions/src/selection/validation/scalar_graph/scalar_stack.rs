//! Exact scalar stack fragments use SSA identity and ordinary frame/pointer instructions.
use super::*;
use crate::selection::scalar_call_abi::{scalar_shape, scalar_stack_placement};
use calling_conventions::ValuePlacement;
use legalized_operations::LegalizedScalarInstruction;
use selected_instructions::{
    FrameStorageSlotId, OutgoingArgumentSlotId, SelectedOutgoingArgumentSlot,
};
use semantic_vocabulary::IntegerType;

fn address_type() -> Result<ScalarType, SelectedInstructionError> {
    IntegerType::new(IntegerSign::Unsigned, 64)
        .map(ScalarType::Integer)
        .map_err(|_| SelectedInstructionError::SourceCustodyMismatch)
}

fn address_register(
    replay: &mut Replay<'_>,
    value: ValueId,
    site: ValueDefinitionSite,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    replay.check_register(
        site,
        address_type()?,
        VirtualRegisterOrigin::ScalarAbiAddress {
            instruction: SelectedInstructionId(
                replay
                    .instruction_cursor
                    .try_into()
                    .map_err(|_| replay.invalid())?,
            ),
            source_value: value,
        },
        None,
    )
}

pub(super) fn entry(
    source: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let accepts_stack_parameters =
        crate::selection::scalar_call_abi::accepts_stack_parameter_entry(source);
    for (parameter_index, parameter) in source.parameters.iter().enumerate() {
        if !replay.required_values.contains(&parameter.value) {
            continue;
        }
        let Some((abi_stack_byte_offset, byte_size, _)) =
            scalar_stack_placement(&parameter.placement)
        else {
            continue;
        };
        if !accepts_stack_parameters
            || source.ranked.is_some()
            || scalar_shape(parameter.scalar_type) != Some(parameter.placement.shape)
            || source.call_plan.parameters.get(parameter_index) != Some(&parameter.placement)
        {
            return Err(invalid());
        }
        let provenance = SelectedInstructionProvenance {
            values: vec![parameter.value],
            ..Default::default()
        };
        let address = address_register(replay, parameter.value, parameter.definition_site)?;
        replay.check_instruction(
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Incoming {
                    parameter_index: parameter_index.try_into().map_err(|_| invalid())?,
                    abi_stack_byte_offset,
                },
                byte_offset: 0,
            },
            replay.constraints.keys.frame_address.ok_or_else(invalid)?,
            &[address],
            &provenance,
        )?;
        let value = replay.result_register(
            parameter.value,
            parameter.definition_site,
            parameter.scalar_type,
        )?;
        let (kind, key) = match byte_size {
            1 => (
                SelectedInstructionKind::Load8 { byte_offset: 0 },
                replay.constraints.keys.load8,
            ),
            2 => (
                SelectedInstructionKind::Load16 { byte_offset: 0 },
                replay.constraints.keys.load16,
            ),
            4 => (
                SelectedInstructionKind::Load32 { byte_offset: 0 },
                replay.constraints.keys.load32,
            ),
            8 => (
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                replay.constraints.keys.load64,
            ),
            _ => return Err(invalid()),
        };
        replay.check_instruction(
            kind,
            key.ok_or_else(invalid)?,
            &[address, value],
            &provenance,
        )?;
        replay.definitions.push((
            parameter.value,
            value,
            parameter.definition_site,
            parameter.scalar_type,
        ));
    }
    Ok(())
}

pub(super) fn argument(
    replay: &mut Replay<'_>,
    operation: &LegalizedScalarInstruction,
    argument_index: usize,
    value: ValueId,
    placement: &ValuePlacement,
) -> Result<bool, SelectedInstructionError> {
    let Some((abi_stack_byte_offset, byte_size, alignment)) = scalar_stack_placement(placement)
    else {
        return Ok(false);
    };
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let (_, input, site, scalar_type) = replay.resolve(value).ok_or_else(invalid)?;
    if scalar_shape(scalar_type) != Some(placement.shape) {
        return Err(invalid());
    }
    let slot = OutgoingArgumentSlotId {
        operation: operation.operation,
        argument_index: argument_index.try_into().map_err(|_| invalid())?,
    };
    replay.transport.slots.push(SelectedOutgoingArgumentSlot {
        id: slot,
        byte_size: u32::from(byte_size),
        alignment,
        abi_stack_byte_offset,
    });
    let provenance = SelectedInstructionProvenance {
        operations: vec![operation.operation],
        values: vec![value],
        ..Default::default()
    };
    let address = address_register(replay, value, site)?;
    replay.check_instruction(
        SelectedInstructionKind::FrameAddress {
            slot: FrameStorageSlotId::Outgoing(slot),
            byte_offset: 0,
        },
        replay.constraints.keys.frame_address.ok_or_else(invalid)?,
        &[address],
        &provenance,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: byte_size as u8,
        },
        replay.constraints.keys.store.ok_or_else(invalid)?,
        &[address, input],
        &provenance,
    )?;
    Ok(true)
}
