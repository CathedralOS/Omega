//! Entry register preservation in the existing Unit frame.

use crate::assignment::shared::*;

#[cfg(test)]
mod tests;

pub(super) fn assign(
    body: &target_operations::TargetUnitBody,
    target: NativeTarget,
    cursor: &mut u32,
) -> Result<Vec<assigned_target_operations::EntryRegisterSpill>, AssignmentError> {
    let mut spills = Vec::new();
    if !body
        .operations
        .iter()
        .any(|operation| matches!(operation, TargetUnitOperation::Continue { .. }))
    {
        return Ok(spills);
    }
    for (parameter_index, parameter) in body.scalar_parameters.iter().enumerate() {
        super::scalar_call::validate_placement_registers(
            parameter.value,
            &parameter.placement,
            target,
        )?;
        if body.call_plan.parameters.get(parameter_index) != Some(&parameter.placement) {
            return Err(AssignmentError::UnitScalarCallSourceMismatch(
                parameter.value,
            ));
        }
        match parameter.placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] if *byte_size == parameter.placement.shape.byte_size => {
                *cursor = super::scalar_call::align_unit_frame_offset(*cursor, 8)?;
                spills.push(assigned_target_operations::EntryRegisterSpill {
                    source_value: parameter.value,
                    parameter_index,
                    register: *register,
                    byte_offset: *cursor,
                });
                *cursor = cursor
                    .checked_add(8)
                    .ok_or(AssignmentError::UnitScalarFrameNotEncodable)?;
            }
            [
                ValueLocation::Stack {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] if *byte_size == parameter.placement.shape.byte_size => {}
            _ => {
                return Err(AssignmentError::UnitScalarCallSourceMismatch(
                    parameter.value,
                ));
            }
        }
    }
    Ok(spills)
}

pub(super) fn parameter_source(
    body: &target_operations::TargetUnitBody,
    target: NativeTarget,
    source: TargetUnitScalarArgumentSource,
) -> Option<AssignedUnitScalarArgumentSource> {
    let TargetUnitScalarArgumentSource::Parameter {
        parameter_index,
        source_value,
        scalar_type,
    } = source
    else {
        return None;
    };
    if !body
        .operations
        .iter()
        .any(|operation| matches!(operation, TargetUnitOperation::Continue { .. }))
    {
        return None;
    }
    let index = usize::try_from(parameter_index).ok()?;
    let parameter = body.scalar_parameters.get(index)?;
    if parameter.value != source_value || parameter.scalar_type != scalar_type {
        return None;
    }
    let mut cursor = super::scalar_call::unit_scalar_home_start(body, target).ok()?;
    let spills = assign(body, target, &mut cursor).ok()?;
    let location = if let Some(spill) = spills.iter().find(|spill| spill.parameter_index == index) {
        AssignedScalarLocation::FrameSpill {
            byte_offset: spill.byte_offset,
        }
    } else if let [
        ValueLocation::Stack {
            stack_byte_offset, ..
        },
    ] = parameter.placement.locations.as_slice()
    {
        AssignedScalarLocation::IncomingStack {
            byte_offset: *stack_byte_offset,
        }
    } else {
        return None;
    };
    Some(AssignedUnitScalarArgumentSource::Parameter {
        parameter_index,
        source_value,
        scalar_type,
        location,
    })
}
