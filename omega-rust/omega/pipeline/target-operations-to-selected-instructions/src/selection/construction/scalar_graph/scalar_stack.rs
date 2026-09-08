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
    builder: &mut Builder<'_>,
    value: ValueId,
    site: ValueDefinitionSite,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let id = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
    builder.registers.push(VirtualRegister {
        id,
        scalar_type: address_type()?,
        class: builder.class,
        origin: VirtualRegisterOrigin::ScalarAbiAddress {
            instruction: SelectedInstructionId(
                builder
                    .instructions
                    .len()
                    .try_into()
                    .map_err(|_| invalid())?,
            ),
            source_value: value,
        },
        definition_site: Some(site),
        entry_fixed_view: None,
    });
    Ok(id)
}

pub(super) fn entry(
    source: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    for (parameter_index, parameter) in source.parameters.iter().enumerate() {
        if !builder.required_values.contains(&parameter.value) {
            continue;
        }
        let Some((abi_stack_byte_offset, byte_size, _)) =
            scalar_stack_placement(&parameter.placement)
        else {
            continue;
        };
        if source.call_plan.result.is_some()
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
        let address = address_register(builder, parameter.value, parameter.definition_site)?;
        builder.emit(
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Incoming {
                    parameter_index: parameter_index.try_into().map_err(|_| invalid())?,
                    abi_stack_byte_offset,
                },
                byte_offset: 0,
            },
            builder.constraints.keys.frame_address.ok_or_else(invalid)?,
            &[address],
            provenance.clone(),
        )?;
        let value = builder.register(
            parameter.value,
            parameter.definition_site,
            parameter.scalar_type,
        )?;
        let (kind, key) = match byte_size {
            4 => (
                SelectedInstructionKind::Load32 { byte_offset: 0 },
                builder.constraints.keys.load32,
            ),
            8 => (
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                builder.constraints.keys.load64,
            ),
            _ => return Err(invalid()),
        };
        builder.emit(
            kind,
            key.ok_or_else(invalid)?,
            &[address, value],
            provenance.clone(),
        )?;
        builder.definitions.push((
            parameter.value,
            value,
            parameter.definition_site,
            parameter.scalar_type,
        ));
    }
    Ok(())
}

pub(super) fn argument(
    builder: &mut Builder<'_>,
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
    let (_, input, site, scalar_type) = builder.resolve(value).ok_or_else(invalid)?;
    if scalar_shape(scalar_type) != Some(placement.shape) {
        return Err(invalid());
    }
    let slot = OutgoingArgumentSlotId {
        operation: operation.operation,
        argument_index: argument_index.try_into().map_err(|_| invalid())?,
    };
    builder.transport.slots.push(SelectedOutgoingArgumentSlot {
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
    let address = address_register(builder, value, site)?;
    builder.emit(
        SelectedInstructionKind::FrameAddress {
            slot: FrameStorageSlotId::Outgoing(slot),
            byte_offset: 0,
        },
        builder.constraints.keys.frame_address.ok_or_else(invalid)?,
        &[address],
        provenance.clone(),
    )?;
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: byte_size as u8,
        },
        builder.constraints.keys.store.ok_or_else(invalid)?,
        &[address, input],
        provenance.clone(),
    )?;
    Ok(true)
}
