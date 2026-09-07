//! Independent reconstruction of a direct ordinary structural result store.

use calling_conventions::{ValueClass, ValueLocation};
use machine_code::{InternalUnitCallRecord, UnitParameterHomeRecord, UnitScalarHomeRecord};
use target::{Architecture, NativeTarget};
use target_operations::{CallSiteOwner, TargetStructuralHomeLayout};

pub(crate) fn expected_store_bytes(
    target: NativeTarget,
    call: &InternalUnitCallRecord,
) -> Option<Vec<u8>> {
    let result = call.structural_result.as_ref()?;
    let home = result.result_home.as_ref()?;
    let TargetStructuralHomeLayout::Aggregate(shape) = home.requirement.layout else {
        return None;
    };
    if call.owner != CallSiteOwner::Operation(home.requirement.defining_operation)
        || home.requirement.result != result.operation_result
        || result.operation_result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
        || !result.operation_result.qualifications.is_empty()
        || !result.operation_result.projected_qualifications.is_empty()
        || !result.operation_result.claims.is_empty()
        || !result.returned_claim_transfers.is_empty()
        || !result.returned_claims.is_empty()
        || !call.scalar_arguments.is_empty()
        || shape.class != ValueClass::Integer
        || !shape.alignment.is_power_of_two()
        || !(shape.byte_size == 8 && shape.alignment == 8 || (9..=16).contains(&shape.byte_size))
        || result.caller_result_placement != result.callee_result_placement
        || result.caller_result_placement.shape != shape
        || !home
            .home_byte_offset
            .is_multiple_of(u32::from(shape.alignment.max(8)))
    {
        return None;
    }
    let mut bytes = Vec::new();
    let mut cursor = 0_u16;
    for location in &result.caller_result_placement.locations {
        let ValueLocation::Register {
            register,
            value_byte_offset,
            byte_size,
        } = *location
        else {
            return None;
        };
        if value_byte_offset != cursor || !(1..=8).contains(&byte_size) {
            return None;
        }
        let offset = home.home_byte_offset.checked_add(u32::from(cursor))?;
        match target.architecture {
            Architecture::X86_64 if super::packed_fragment::is_packed(byte_size) => {
                super::packed_fragment::x86_store(
                    &mut bytes,
                    super::x86_terminal_register(register)?,
                    offset,
                    byte_size,
                )?;
            }
            Architecture::Aarch64 if super::packed_fragment::is_packed(byte_size) => {
                super::packed_fragment::aarch64_store(
                    &mut bytes,
                    super::aarch64_terminal_register(register)?,
                    offset,
                    byte_size,
                )?;
            }
            Architecture::X86_64 => super::projected_copy::x86_stack_store(
                &mut bytes,
                super::x86_terminal_register(register)?,
                offset,
                byte_size,
            )?,
            Architecture::Aarch64 => bytes.extend_from_slice(
                &super::projected_copy::aarch64_stack_store(
                    super::aarch64_terminal_register(register)?,
                    offset,
                    byte_size,
                )?
                .to_le_bytes(),
            ),
        }
        cursor = cursor.checked_add(byte_size)?;
    }
    (cursor == shape.byte_size).then_some(bytes)
}

pub(crate) fn parameter_storage_end(
    target: NativeTarget,
    parameter_homes: &[UnitParameterHomeRecord],
    scalar_abi: Option<&machine_code::UnitScalarFunctionAbiRecord>,
) -> Option<u32> {
    let scalar_parameters = scalar_abi.map_or(&[][..], |abi| abi.parameters.as_slice());
    let scalar_shapes = scalar_parameters
        .iter()
        .map(|parameter| {
            let shape = crate::unit_scalar_call_custody::scalar_home_shape(parameter.scalar_type)?;
            (parameter.placement.shape == shape).then_some(shape)
        })
        .collect::<Option<Vec<_>>>()?;
    let Ok(caller_plan) = calling_conventions::evaluate_call_plan(
        calling_conventions::CallingPolicy::native_for_target(target),
        &calling_conventions::CallSignature {
            parameters: scalar_shapes
                .into_iter()
                .chain(parameter_homes.iter().map(|parameter| parameter.shape))
                .collect(),
            result: None,
        },
    ) else {
        return None;
    };
    if caller_plan.parameters.len() != scalar_parameters.len() + parameter_homes.len()
        || scalar_abi.is_some_and(|abi| abi.call_plan != caller_plan)
        || scalar_parameters
            .iter()
            .zip(&caller_plan.parameters)
            .any(|(parameter, placement)| parameter.placement != *placement)
        || caller_plan.parameters[scalar_parameters.len()..]
            .iter()
            .zip(parameter_homes)
            .any(|(placement, parameter)| {
                placement != &parameter.source
                    || parameter.indirect
                        != matches!(
                            parameter.source.locations.as_slice(),
                            [ValueLocation::Indirect { .. }]
                        )
            })
    {
        return None;
    }
    let mut parameter_cursor = 0_u32;
    for parameter in parameter_homes {
        let alignment = match target.architecture {
            Architecture::X86_64 => 8,
            Architecture::Aarch64 => u32::from(parameter.shape.alignment.clamp(8, 16)),
        };
        let offset = parameter_cursor.checked_next_multiple_of(alignment)?;
        if parameter.location.stack_byte_offset() != Some(offset) {
            return None;
        }
        let end = offset.checked_add(if parameter.indirect {
            8
        } else {
            u32::from(parameter.shape.byte_size)
        })?;
        parameter_cursor = end;
    }
    Some(parameter_cursor)
}

pub(crate) fn exact_frame(
    target: NativeTarget,
    storage_end: u32,
    frame_bytes: u32,
    return_link: Option<u32>,
) -> bool {
    let expected_link = storage_end.checked_next_multiple_of(8);
    let expected_frame = match target.architecture {
        Architecture::X86_64 => storage_end.checked_next_multiple_of(16),
        Architecture::Aarch64 => expected_link
            .and_then(|offset| offset.checked_add(8))
            .and_then(|size| size.checked_next_multiple_of(16)),
    };
    expected_frame == Some(frame_bytes)
        && return_link.is_none_or(|offset| Some(offset) == expected_link)
}

pub(crate) fn exact_storage(
    target: NativeTarget,
    call: &InternalUnitCallRecord,
    calls: &[InternalUnitCallRecord],
    frame_bytes: u32,
    parameter_homes: &[UnitParameterHomeRecord],
    scalar_homes: &[UnitScalarHomeRecord],
    scalar_abi: Option<&machine_code::UnitScalarFunctionAbiRecord>,
    return_link: Option<u32>,
    has_continuations: bool,
) -> bool {
    let Some(entry_end) = crate::unit_scalar_call_custody::entry_spills::storage_end(
        target,
        parameter_homes,
        scalar_abi,
        has_continuations,
    ) else {
        return false;
    };
    if !scalar_homes.is_empty() {
        return false;
    }
    let Some(result) = &call.structural_result else {
        return false;
    };
    let Some(home) = &result.result_home else {
        return false;
    };
    let Some(expected) = expected_store_bytes(target, call) else {
        return false;
    };
    let Some(end) = home
        .home_byte_offset
        .checked_add(u32::from(home.requirement.layout.shape().byte_size))
    else {
        return false;
    };
    let overlaps = |offset: u32, width: u32| {
        offset
            .checked_add(width)
            .is_none_or(|other_end| home.home_byte_offset < other_end && offset < end)
    };
    let parameter_end = parameter_homes
        .iter()
        .map(|parameter| {
            parameter
                .location
                .stack_byte_offset()?
                .checked_add(if parameter.indirect {
                    8
                } else {
                    u32::from(parameter.shape.byte_size)
                })
        })
        .collect::<Option<Vec<_>>>();
    let Some(parameter_end) = parameter_end else {
        return false;
    };
    let mut result_end = entry_end;
    let mut found = false;
    let mut previous_ordinal = None;
    for producer in calls {
        if previous_ordinal.is_some_and(|ordinal| ordinal >= producer.operation_ordinal) {
            return false;
        }
        previous_ordinal = Some(producer.operation_ordinal);
        let Some(stored) = producer
            .structural_result
            .as_ref()
            .and_then(|result| result.result_home.as_ref())
        else {
            continue;
        };
        let shape = stored.requirement.layout.shape();
        if result_end.checked_next_multiple_of(u32::from(shape.alignment.max(8)))
            != Some(stored.home_byte_offset)
            || expected_store_bytes(target, producer).as_ref() != Some(&stored.bytes)
            || stored.byte_count != stored.bytes.len()
        {
            return false;
        }
        let Some(end) = stored
            .home_byte_offset
            .checked_add(u32::from(shape.byte_size))
        else {
            return false;
        };
        result_end = end;
        if producer.owner == call.owner {
            if producer != call || found {
                return false;
            }
            found = true;
        }
    }
    if !found {
        return false;
    }
    let scalar_end = scalar_homes
        .iter()
        .map(|scalar| {
            scalar
                .byte_offset
                .checked_add(u32::from(scalar.shape.byte_size))
        })
        .collect::<Option<Vec<_>>>();
    let Some(scalar_end) = scalar_end else {
        return false;
    };
    let live_end = parameter_end
        .into_iter()
        .chain(scalar_end)
        .chain(std::iter::once(result_end))
        .max()
        .unwrap_or(end);
    home.byte_count == expected.len()
        && home.bytes == expected
        && home.code_offset >= call.code_offset
        && home.code_offset.checked_add(home.byte_count)
            == call.code_offset.checked_add(call.byte_count)
        && end <= frame_bytes
        && exact_frame(target, live_end, frame_bytes, return_link)
        && parameter_homes.iter().all(|parameter| {
            parameter
                .location
                .stack_byte_offset()
                .is_some_and(|offset| {
                    !overlaps(
                        offset,
                        if parameter.indirect {
                            8
                        } else {
                            u32::from(parameter.shape.byte_size)
                        },
                    )
                })
        })
        && scalar_homes
            .iter()
            .all(|scalar| !overlaps(scalar.byte_offset, u32::from(scalar.shape.byte_size)))
        && return_link.is_none_or(|offset| !overlaps(offset, 8))
}
