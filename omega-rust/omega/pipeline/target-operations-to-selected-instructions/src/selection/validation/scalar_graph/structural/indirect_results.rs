//! Hidden result storage uses the declared result place and the native CallPlan.
use super::*;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};

pub(in crate::selection) fn entry(
    source: &LegalizedScalarFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let Some(placement) = source.call_plan.result.as_ref() else {
        return Ok(());
    };
    let Some(machine_register) = crate::selection::aggregate_result_input::indirect_result(
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
        .fixed_register_view(machine_register)
        .ok_or_else(invalid)?;
    let instruction = SelectedInstructionId(
        replay
            .instruction_cursor
            .try_into()
            .map_err(|_| replay.invalid())?,
    );
    let input = register(
        replay,
        VirtualRegisterOrigin::AbiTransport {
            instruction,
            place: result.place,
            byte_offset: 0,
        },
        Some(fixed),
    )?;
    let retained = super::result(replay, result.place, 0)?;
    replay.check_instruction(
        SelectedInstructionKind::CopyI64,
        replay.constraints.keys.copy_i64,
        &[input, retained],
        &Default::default(),
    )?;
    replay.transport.result_pointer = Some(retained);
    Ok(())
}

pub(in crate::selection) fn prepare_call(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    replay: &mut Replay<'_>,
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
    if replay
        .transport
        .local_slots
        .iter()
        .any(|existing| existing.id == slot)
    {
        return Err(invalid());
    }
    replay.transport.local_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size: u32::from(placement.shape.byte_size),
        alignment: placement.shape.alignment,
    });
    // Reserve the destination before the call, but publish no initialized value yet.
    let pointer = local_storage::address(
        replay,
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
    replay: &mut Replay<'_>,
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
        replay,
        row,
        slot,
        0,
        u32::from(placement.shape.byte_size),
        false,
    )?;
    replay.transport.pointers.push((result.place, pointer));
    Ok(())
}

pub(in crate::selection) fn returned(
    source: &LegalizedScalarFunction,
    block: &legalized_operations::LegalizedScalarBlock,
    returned: &legalized_operations::LegalizedScalarReturn,
    slot: LocalStorageSlotId,
    placement: &calling_conventions::ValuePlacement,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let destination = source
        .structural
        .as_ref()
        .and_then(|signature| signature.result.as_ref())
        .ok_or_else(invalid)?
        .place;
    let pointer = replay.transport.result_pointer.ok_or_else(invalid)?;
    let place = slot.structural_place().ok_or_else(invalid)?;
    let input = super::result(replay, place, 0)?;
    super::super::structural_case::memory(
        replay,
        block.id,
        place,
        0,
        u32::from(placement.shape.byte_size),
        SelectedMemoryAccessRole::AddressLocal { slot },
    )?;
    replay.check_instruction(
        SelectedInstructionKind::FrameAddress {
            slot: selected_instructions::FrameStorageSlotId::Local(slot),
            byte_offset: 0,
        },
        replay.constraints.keys.frame_address.ok_or_else(invalid)?,
        &[input],
        &Default::default(),
    )?;
    let mut offset = 0u32;
    while offset < u32::from(placement.shape.byte_size) {
        let width = (u32::from(placement.shape.byte_size) - offset).min(8) as u16;
        let value = super::result(replay, place, offset)?;
        super::super::structural_case::memory(
            replay,
            block.id,
            place,
            offset,
            u32::from(width),
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        super::super::aggregate_memory::load(replay, input, value, offset, width)?;
        super::super::structural_case::memory(
            replay,
            block.id,
            destination,
            offset,
            u32::from(width),
            SelectedMemoryAccessRole::WritePlace,
        )?;
        super::super::aggregate_memory::store(replay, pointer, value, offset, width)?;
        offset += u32::from(width);
    }
    let returned_pointer = matches!(
        source.call_plan.policy,
        calling_conventions::CallingPolicy::MicrosoftX64
            | calling_conventions::CallingPolicy::SystemVAMD64
    );
    let pointer = if returned_pointer {
        let output = super::result(replay, destination, 0)?;
        replay.check_instruction(
            SelectedInstructionKind::CopyI64,
            replay.constraints.keys.copy_i64,
            &[pointer, output],
            &Default::default(),
        )?;
        output
    } else {
        pointer
    };
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
    let kind = if returned_pointer {
        SelectedInstructionKind::ReturnScalar
    } else {
        SelectedInstructionKind::ReturnUnit
    };
    let key = if returned_pointer {
        replay.constraints.keys.return_i64
    } else {
        replay.constraints.keys.return_unit
    };
    let operands = if returned_pointer {
        std::slice::from_ref(&pointer)
    } else {
        &[]
    };
    if *psi_return_edge != returned.edge
        || instruction.id.0 as usize != replay.instruction_cursor
        || instruction.kind != kind
        || instruction.constraint != key
        || instruction.provenance != provenance
        || instruction
            .operands
            .iter()
            .map(|operand| operand.virtual_register)
            .ne(operands.iter().copied())
    {
        return Err(invalid());
    }
    Ok(())
}

fn invalid() -> SelectedInstructionError {
    SelectedInstructionError::SourceCustodyMismatch
}
