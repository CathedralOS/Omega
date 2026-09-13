//! Hidden result storage uses the declared result place and the native CallPlan.
use super::*;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};

pub(in crate::selection) fn entry(
    source: &LegalizedScalarFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let Some(placement) = source.call_plan.result.as_ref() else {
        return Ok(());
    };
    let Some(register) = crate::selection::aggregate_result_input::indirect_result(
        placement,
        source.call_plan.policy,
    ) else {
        return Ok(());
    };
    if source.call_plan.policy
        != calling_conventions::CallingPolicy::native_for_target(environment.target())
    {
        return Err(invalid());
    }
    let signature = source.structural.as_ref().ok_or_else(invalid)?;
    let result = signature.result.as_ref().ok_or_else(invalid)?;
    if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || crate::structural_reference_input::shape(
            result.structural_type,
            &signature.structural_types,
        ) != Some(placement.shape)
    {
        return Err(invalid());
    }
    let fixed = environment
        .fixed_register_view(register)
        .ok_or_else(invalid)?;
    let input = transport_register(builder, result.place, 0)?;
    builder.registers[input.0 as usize].entry_fixed_view = Some(fixed);
    let retained = transport_register(builder, result.place, 0)?;
    builder.emit(
        SelectedInstructionKind::CopyI64,
        builder.constraints.keys.copy_i64,
        &[input, retained],
        Default::default(),
    )?;
    builder.transport.result_pointer = Some(retained);
    Ok(())
}

pub(in crate::selection) fn prepare_call(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::Call(call) = &row.kind else {
        return Err(invalid());
    };
    let Some(placement) = call.result_placement.as_ref() else {
        return Ok(None);
    };
    if crate::selection::aggregate_result_input::indirect_result(placement, call.call_plan.policy)
        .is_none()
    {
        return Ok(None);
    }
    let (result, placement) =
        crate::selection::aggregate_result_input::call_result(source, call).ok_or_else(invalid)?;
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: result.place,
    };
    if builder
        .transport
        .local_slots
        .iter()
        .any(|existing| existing.id == slot)
    {
        return Err(invalid());
    }
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: u32::from(placement.shape.byte_size),
            alignment: placement.shape.alignment,
        });
    // Reserve the destination before the call, but publish no initialized value yet.
    let pointer = local_storage::address(
        builder,
        row,
        slot,
        0,
        u32::from(placement.shape.byte_size),
        false,
    )?;
    Ok(Some(pointer))
}

pub(in crate::selection) fn finish_call(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::Call(call) = &row.kind else {
        return Err(invalid());
    };
    let (result, placement) =
        crate::selection::aggregate_result_input::call_result(source, call).ok_or_else(invalid)?;
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: result.place,
    };
    let pointer = local_storage::address(
        builder,
        row,
        slot,
        0,
        u32::from(placement.shape.byte_size),
        false,
    )?;
    builder.transport.pointers.push((result.place, pointer));
    Ok(())
}

pub(in crate::selection) fn returned(
    source: &LegalizedScalarFunction,
    block: &legalized_operations::LegalizedScalarBlock,
    returned: &legalized_operations::LegalizedScalarReturn,
    place: PlaceId,
    input: VirtualRegisterId,
    placement: &calling_conventions::ValuePlacement,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let destination = source
        .structural
        .as_ref()
        .and_then(|signature| signature.result.as_ref())
        .ok_or_else(invalid)?
        .place;
    let pointer = builder.transport.result_pointer.ok_or_else(invalid)?;
    let mut offset = 0u32;
    while offset < u32::from(placement.shape.byte_size) {
        let width = (u32::from(placement.shape.byte_size) - offset).min(8) as u16;
        let value = transport_register(builder, place, offset)?;
        super::super::structural_case::memory(
            builder,
            block.id,
            place,
            offset,
            u32::from(width),
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        super::super::aggregate_memory::load(builder, input, value, offset, width)?;
        super::super::structural_case::memory(
            builder,
            block.id,
            destination,
            offset,
            u32::from(width),
            SelectedMemoryAccessRole::WritePlace,
        )?;
        super::super::aggregate_memory::store(builder, pointer, value, offset, width)?;
        offset += u32::from(width);
    }
    let returned_pointer = matches!(
        source.call_plan.policy,
        calling_conventions::CallingPolicy::MicrosoftX64
            | calling_conventions::CallingPolicy::SystemVAMD64
    );
    // The ABI return use must not pin the retained pointer across earlier calls.
    let pointer = if returned_pointer {
        let output = transport_register(builder, destination, 0)?;
        builder.emit(
            SelectedInstructionKind::CopyI64,
            builder.constraints.keys.copy_i64,
            &[pointer, output],
            Default::default(),
        )?;
        output
    } else {
        pointer
    };
    builder.emit(
        if returned_pointer {
            SelectedInstructionKind::ReturnScalar
        } else {
            SelectedInstructionKind::ReturnUnit
        },
        if returned_pointer {
            builder.constraints.keys.return_i64
        } else {
            builder.constraints.keys.return_unit
        },
        if returned_pointer {
            std::slice::from_ref(&pointer)
        } else {
            &[]
        },
        SelectedInstructionProvenance {
            edges: vec![returned.edge],
            fuel: returned.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(())
}
