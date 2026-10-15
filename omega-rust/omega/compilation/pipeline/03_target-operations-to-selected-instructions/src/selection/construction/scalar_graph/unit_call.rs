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
        .map_err(|reason| {
            SelectedInstructionError::custody_because(match reason {
                legalized_operations::LegalizedCallSourceError::ProviderIdentityMismatch => {
                    "call provider identity differs from its installed source"
                }
                legalized_operations::LegalizedCallSourceError::ArgumentSignatureMismatch => {
                    "call arguments differ from the installed provider signature"
                }
                legalized_operations::LegalizedCallSourceError::CompletionEvidenceMismatch => {
                    "call completion evidence differs from its source"
                }
                legalized_operations::LegalizedCallSourceError::OwnershipMismatch => {
                    "call ownership events differ from its source transfers"
                }
            })
        })?;
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
    if let Some(selector) = call.target.selector() {
        let (_, input, site, scalar_type) = builder
            .resolve(selector)
            .ok_or(SelectedInstructionError::custody())?;
        if scalar_type != target_operations::selector_carrier_type() {
            return Err(SelectedInstructionError::custody());
        }
        let dispatch = builder.copy(input, selector, site, scalar_type)?;
        let inputs = operands
            .len()
            .checked_sub(result_registers.len())
            .ok_or(SelectedInstructionError::custody())?;
        operands.insert(inputs, dispatch);
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
                target: selected_instructions::SelectedCallTarget::from_native(call.target),
            }
        } else {
            SelectedInstructionKind::CallUnit {
                target: selected_instructions::SelectedCallTarget::from_native(call.target),
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
                .chain(call.target.selector())
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
        use selected_instructions::SelectedMemoryAccessRole;
        let slot = builder.transport.push_structural_local_slot(
            operation.operation,
            result.place,
            placement.shape.byte_size,
            placement.shape.alignment,
        );
        let block = super::block_containing_operation(source, operation.operation)?;
        let pointer = super::structural_case::register(builder, result.place, 0, 64, false)?;
        super::structural_case::memory(
            builder,
            block,
            result.place,
            0,
            placement.shape.byte_size,
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        builder.emit_local_frame_address(slot, pointer, Default::default())?;
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
                    key.ok_or(SelectedInstructionError::custody())?,
                    &[value, output],
                    Default::default(),
                )?;
                output
            } else {
                value
            };
            let (pointer, displacement) = super::structural::displaced_memory_pointer(
                builder,
                result.place,
                pointer,
                *value_byte_offset,
                *byte_size,
                super::structural::provenance(operation),
            )?;
            super::structural_case::memory(
                builder,
                block,
                result.place,
                *value_byte_offset,
                *byte_size,
                SelectedMemoryAccessRole::WritePlace,
            )?;
            super::aggregate_memory::store(builder, pointer, value, displacement, *byte_size)?;
        }
        // Later state edges use this initialized result home through the ordinary
        // place roster; they must not allocate another home or retain ABI registers.
        builder.transport.pointers.push((result.place, pointer));
    }
    Ok(())
}
