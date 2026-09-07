//! Replay and realize entry preservation before any Unit operation can clobber it.

use super::*;

#[cfg(test)]
mod tests;

pub(super) fn validate(
    cursor: &mut u32,
    body: &AssignedUnitBody,
    target: NativeTarget,
) -> Result<(), EmissionError> {
    let invalid = || EmissionError::UnitCallStackAreaNotEncodable;
    if !body
        .operations
        .iter()
        .any(|operation| matches!(operation, AssignedUnitOperation::Continue { .. }))
    {
        return if body.entry_register_spills.is_empty() {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    let scalar_shapes = body
        .scalar_parameters
        .iter()
        .map(|parameter| {
            let shape = unit_scalar_shape(parameter.value, parameter.scalar_type)?;
            (shape == parameter.placement.shape)
                .then_some(shape)
                .ok_or_else(invalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected_plan = calling_conventions::evaluate_call_plan(
        calling_conventions::CallingPolicy::native_for_target(target),
        &calling_conventions::CallSignature {
            parameters: scalar_shapes
                .into_iter()
                .chain(body.parameters.iter().map(|parameter| parameter.shape))
                .collect(),
            result: None,
        },
    )
    .map_err(|_| invalid())?;
    if body.call_plan != expected_plan
        || !body
            .scalar_parameters
            .iter()
            .map(|parameter| &parameter.placement)
            .chain(body.parameters.iter().map(|parameter| &parameter.placement))
            .eq(expected_plan.parameters.iter())
    {
        return Err(invalid());
    }
    let mut expected = Vec::new();
    for (parameter_index, parameter) in body.scalar_parameters.iter().enumerate() {
        match parameter.placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] if *byte_size == parameter.placement.shape.byte_size => {
                *cursor = align_u32(*cursor, 8)?;
                expected.push(assigned_target_operations::EntryRegisterSpill {
                    source_value: parameter.value,
                    parameter_index,
                    register: *register,
                    byte_offset: *cursor,
                });
                *cursor = cursor.checked_add(8).ok_or_else(invalid)?;
            }
            [
                ValueLocation::Stack {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] if *byte_size == parameter.placement.shape.byte_size => {}
            _ => return Err(invalid()),
        }
    }
    if expected != body.entry_register_spills {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn emit(
    bytes: &mut Vec<u8>,
    target: NativeTarget,
    spills: &[assigned_target_operations::EntryRegisterSpill],
) -> Result<Vec<machine_code::UnitEntryRegisterSpillRecord>, EmissionError> {
    let mut records = Vec::with_capacity(spills.len());
    for spill in spills {
        let code_offset = bytes.len();
        match target.architecture {
            Architecture::X86_64 => emit_x86_64_stack_store_width(
                bytes,
                x86_unit_register(spill.register)?,
                spill.byte_offset,
                8,
            )?,
            Architecture::Aarch64 => {
                let instruction = aarch64_unit_stack_access(
                    aarch64_store_base(8)?,
                    aarch64_unit_register(spill.register)?,
                    spill.byte_offset,
                    8,
                )?;
                append_aarch64_instructions(bytes, vec![instruction]);
            }
        }
        records.push(machine_code::UnitEntryRegisterSpillRecord {
            source_value: spill.source_value,
            parameter_index: spill.parameter_index,
            register: spill.register,
            byte_offset: spill.byte_offset,
            code_offset,
            byte_count: bytes.len() - code_offset,
        });
    }
    Ok(records)
}
