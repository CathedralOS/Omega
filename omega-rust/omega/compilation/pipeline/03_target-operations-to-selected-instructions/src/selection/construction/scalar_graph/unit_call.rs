//! Resultless and aggregate-result calls marshal their arguments and publish their result home.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstructionKind, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, ValueLocation,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::row;
use legalized_operations::LegalizedScalarInstruction;
use selected_instructions::SelectedCallContract;

pub(super) fn emit(
    function: usize,
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::Call(call) = &operation.kind else {
        return Err(SelectedInstructionError::custody());
    };
    if operation.result.is_some()
        || (call.result_placement.is_some() && call.structural_result.is_none())
    {
        return Err(SelectedInstructionError::custody());
    }
    call.validate_source(&operation.ownership)
        .map_err(|_| SelectedInstructionError::custody())?;
    let key = crate::selection::scalar_call_abi::unit_key(call, environment)
        .ok_or_else(|| SelectedInstructionError::custody())?;
    crate::selection::scalar_call_abi::validate(
        function,
        source,
        call,
        operation.operation,
        key,
        row(builder.catalog, key)?,
        environment,
    )?;
    let mut operands = super::call_arguments::marshal(
        source,
        operation,
        call,
        builder,
        SelectedInstructionError::custody,
    )?;
    let hidden = super::structural::indirect_results::prepare_call(source, operation, builder)?;
    let mut result_registers = Vec::new();
    if let Some(pointer) = hidden {
        operands.push(pointer);
    } else if let Some(result) = &call.structural_result {
        for location in &call
            .result_placement
            .as_ref()
            .ok_or_else(|| SelectedInstructionError::custody())?
            .locations
        {
            let ValueLocation::Register {
                value_byte_offset, ..
            } = location
            else {
                return Err(SelectedInstructionError::custody());
            };
            let register = super::structural_case::register(
                builder,
                result.place,
                *value_byte_offset,
                64,
                false,
            )?;
            operands.push(register);
            result_registers.push(register);
        }
    }
    builder.transport.calls.push(SelectedCallContract {
        instruction: SelectedInstructionId(
            builder
                .instructions
                .len()
                .try_into()
                .map_err(|_| SelectedInstructionError::custody())?,
        ),
        owner: target_operations::CallSiteOwner::Operation(operation.operation),
        call: call.clone(),
        effect: operation.effect,
        ownership: operation.ownership.clone(),
    });
    builder.emit(
        if call.structural_result.is_some()
            && !call
                .result_placement
                .as_ref()
                .is_some_and(crate::selection::scalar_call_abi::empty_aggregate_placement)
        {
            SelectedInstructionKind::CallAggregate {
                callee: call.callee,
            }
        } else {
            SelectedInstructionKind::CallUnit {
                callee: call.callee,
            }
        },
        key,
        &operands,
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: call
                .arguments
                .iter()
                .filter_map(|argument| argument.scalar_source())
                .collect(),
            obligations: call.requirement_obligations.clone(),
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    if hidden.is_some() {
        return super::structural::indirect_results::finish_call(source, operation, builder);
    }
    retain_result(source, operation, &result_registers, builder)
}

/// Capture completed register fragments in the ordinary addressable result
/// home. Both call mechanisms use this storage operation after ABI validation.
pub(super) fn retain_result(
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    result_registers: &[selected_instructions::VirtualRegisterId],
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    if let Some((result, placement, _)) =
        crate::selection::aggregate_result_input::call_completion(source, operation)
    {
        // The call and complete structural result remain retained, but an
        // empty payload owns no physical result home or address operation.
        if crate::selection::scalar_call_abi::empty_aggregate_placement(placement) {
            return Ok(());
        }
        use selected_instructions::{
            FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot,
            SelectedMemoryAccessRole,
        };
        let slot = LocalStorageSlotId::Structural {
            operation: operation.operation,
            place: result.place,
        };
        builder
            .transport
            .local_slots
            .push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: placement.shape.byte_size,
                alignment: placement.shape.alignment,
            });
        let block = source
            .blocks
            .iter()
            .find(|block| {
                block
                    .instructions
                    .iter()
                    .any(|row| row.operation == operation.operation)
            })
            .ok_or_else(|| SelectedInstructionError::custody())?
            .id;
        let pointer = super::structural_case::register(builder, result.place, 0, 64, false)?;
        super::structural_case::memory(
            builder,
            block,
            result.place,
            0,
            placement.shape.byte_size,
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        builder.emit(
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            builder
                .constraints
                .keys
                .frame_address
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[pointer],
            Default::default(),
        )?;
        if placement.locations.len() != result_registers.len() {
            return Err(SelectedInstructionError::custody());
        }
        for (location, &value) in placement.locations.iter().zip(result_registers) {
            let ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            } = location
            else {
                return Err(SelectedInstructionError::custody());
            };
            // Aggregate fragments carry bits, not scalar normalization. Move
            // an FP-bank fragment into an integer scratch before storing its
            // exact byte range in the completed home.
            let value = if builder.registers[value.0 as usize].class != builder.class {
                let (kind, key) = match byte_size {
                    4 => (
                        SelectedInstructionKind::Float32ToBits,
                        builder.constraints.keys.float32_to_bits,
                    ),
                    8 => (
                        SelectedInstructionKind::Float64ToBits,
                        builder.constraints.keys.float64_to_bits,
                    ),
                    _ => return Err(SelectedInstructionError::custody()),
                };
                let output = super::structural_case::register(
                    builder,
                    result.place,
                    *value_byte_offset,
                    64,
                    false,
                )?;
                builder.emit(
                    kind,
                    key.ok_or_else(SelectedInstructionError::custody)?,
                    &[value, output],
                    Default::default(),
                )?;
                output
            } else {
                value
            };
            super::structural_case::memory(
                builder,
                block,
                result.place,
                *value_byte_offset,
                *byte_size,
                SelectedMemoryAccessRole::WritePlace,
            )?;
            super::aggregate_memory::store(
                builder,
                pointer,
                value,
                *value_byte_offset,
                *byte_size,
            )?;
        }
        // Later state edges use this initialized result home through the ordinary
        // place roster; they must not allocate another home or retain ABI registers.
        builder.transport.pointers.push((result.place, pointer));
    }
    Ok(())
}
