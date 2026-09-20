//! Call-site replay for object construction: internal and foreign call
//! relocation fields, foreign scalar argument and callback-address custody,
//! and floating-control preservation around returning foreign calls.

use crate::ObjectError;
use machine_code::{SemanticCodeAttribution, SemanticCodeSite};
use object_file::RelocationKind;
use semantic_vocabulary::MachineId;
use target::{Architecture, NativeTarget};
use target_operations::CallSiteOwner;

pub(crate) fn validate_internal_call_site(
    architecture: Architecture,
    caller: MachineId,
    bytes: &[u8],
    call: machine_code::InternalCallRelocation,
) -> Result<(RelocationKind, usize), ObjectError> {
    let valid = match architecture {
        Architecture::X86_64 => {
            call.offset >= 1
                && bytes.get(call.offset - 1) == Some(&0xe8)
                && bytes.get(call.offset..call.offset.saturating_add(4)) == Some(&[0; 4])
        }
        Architecture::Aarch64 => {
            call.offset.is_multiple_of(4)
                && bytes.get(call.offset..call.offset.saturating_add(4))
                    == Some(&0x9400_0000u32.to_le_bytes())
        }
    };
    if !valid {
        return Err(ObjectError::InvalidInternalCallSite {
            caller,
            owner: call.owner,
            offset: call.offset,
        });
    }
    Ok(match architecture {
        Architecture::X86_64 => (RelocationKind::X86_64Relative32, 4),
        Architecture::Aarch64 => (RelocationKind::Aarch64Branch26, 4),
    })
}

pub(crate) fn validate_foreign_call_site(
    architecture: Architecture,
    caller: MachineId,
    bytes: &[u8],
    call: &machine_code::ForeignCallRelocation,
) -> Result<(RelocationKind, usize), ObjectError> {
    let valid = match architecture {
        Architecture::X86_64 => {
            call.offset >= 1
                && bytes.get(call.offset - 1) == Some(&0xe8)
                && bytes.get(call.offset..call.offset.saturating_add(4)) == Some(&[0; 4])
        }
        Architecture::Aarch64 => {
            call.offset.is_multiple_of(4)
                && bytes.get(call.offset..call.offset.saturating_add(4))
                    == Some(&0x9400_0000u32.to_le_bytes())
        }
    };
    if !valid {
        return Err(ObjectError::InvalidForeignCallSite {
            caller,
            owner: call.owner,
            offset: call.offset,
        });
    }
    Ok(match architecture {
        Architecture::X86_64 => (RelocationKind::X86_64Relative32, 4),
        Architecture::Aarch64 => (RelocationKind::Aarch64Branch26, 4),
    })
}

pub(crate) fn validate_foreign_call_floating_control(
    target: NativeTarget,
    function: &machine_code::MachineCodeFunction,
    call: &machine_code::ForeignCallRelocation,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidForeignCallFloatingControl {
        caller: function.machine,
        owner: call.owner,
    };
    let (
        saved_slot_byte_offset,
        slot_byte_count,
        save_offset,
        save_byte_count,
        restore_offset,
        restore_byte_count,
        expected_save,
        expected_restore,
    ) = match target.architecture {
        Architecture::X86_64 => {
            let Some(control) = call.x86_floating_control else {
                return Err(invalid());
            };
            if call.aarch64_floating_control.is_some() || control.target != target {
                return Err(invalid());
            }
            (
                control.saved_slot_byte_offset,
                4,
                control.save_offset,
                control.save_byte_count,
                control.restore_offset,
                control.restore_byte_count,
                isa_x86_64::encode_stmxcsr_rsp_displacement(control.saved_slot_byte_offset)
                    .map_err(|_| invalid())?,
                isa_x86_64::encode_ldmxcsr_rsp_displacement(control.saved_slot_byte_offset)
                    .map_err(|_| invalid())?,
            )
        }
        Architecture::Aarch64 => {
            let Some(control) = call.aarch64_floating_control else {
                return Err(invalid());
            };
            if call.x86_floating_control.is_some() || control.target != target {
                return Err(invalid());
            }
            (
                control.saved_slot_byte_offset,
                8,
                control.save_offset,
                control.save_byte_count,
                control.restore_offset,
                control.restore_byte_count,
                isa_aarch64::encode_save_fpcr_to_sp_displacement(control.saved_slot_byte_offset)
                    .map_err(|_| invalid())?
                    .to_vec(),
                isa_aarch64::encode_restore_fpcr_from_sp_displacement(
                    control.saved_slot_byte_offset,
                )
                .map_err(|_| invalid())?
                .to_vec(),
            )
        }
    };
    let frame = function
        .unit_stack
        .and_then(|stack| stack.frame)
        .ok_or_else(invalid)?;
    let expected_slot = match target.architecture {
        Architecture::X86_64 => frame
            .byte_size
            .checked_sub(16)
            .and_then(|base| {
                base.checked_add(if function.x86_floating_control.is_some() {
                    8
                } else {
                    0
                })
            })
            .ok_or_else(invalid)?,
        Architecture::Aarch64 => function
            .unit_stack
            .and_then(|stack| stack.aarch64_return_link)
            .and_then(|link| link.frame_byte_offset.checked_sub(8))
            .ok_or_else(invalid)?,
    };
    if saved_slot_byte_offset
        .checked_add(slot_byte_count)
        .is_none_or(|end| end > frame.byte_size)
        || saved_slot_byte_offset != expected_slot
        || function.x86_floating_control.is_some_and(|outer| {
            saved_slot_byte_offset == outer.saved_slot_byte_offset
                || saved_slot_byte_offset == outer.canonical_slot_byte_offset
        })
    {
        return Err(invalid());
    }
    let save_end = save_offset
        .checked_add(save_byte_count)
        .ok_or_else(invalid)?;
    let restore_end = restore_offset
        .checked_add(restore_byte_count)
        .ok_or_else(invalid)?;
    let call_start = match target.architecture {
        Architecture::X86_64 => call.offset.checked_sub(1).ok_or_else(invalid)?,
        Architecture::Aarch64 => call.offset,
    };
    let call_end = call.offset.checked_add(4).ok_or_else(invalid)?;
    let pre_call_start = call.unit_stack.outbound.map_or_else(
        || {
            call.scalar_arguments
                .first()
                .map(|argument| argument.code_offset)
                .into_iter()
                .chain(
                    call.callback_address
                        .as_ref()
                        .map(|callback| callback.code_offset),
                )
                .min()
                .unwrap_or(call_start)
        },
        |outbound| outbound.allocation_offset,
    );
    let post_call_end = match call.unit_stack.outbound {
        Some(outbound) => outbound
            .release_offset
            .checked_add(outbound.release_byte_count)
            .ok_or_else(invalid)?,
        None => call_end,
    };
    if save_byte_count != expected_save.len()
        || restore_byte_count != expected_restore.len()
        || function.bytes.get(save_offset..save_end) != Some(expected_save.as_slice())
        || function.bytes.get(restore_offset..restore_end) != Some(expected_restore.as_slice())
        || save_end != pre_call_start
        || save_offset >= call_start
        || restore_offset != post_call_end
        || restore_offset < call_end
        || call
            .scalar_result
            .as_ref()
            .is_some_and(|result| result.code_offset != restore_end)
    {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn validate_callback_plan_custody(
    target: NativeTarget,
    caller: MachineId,
    call: &machine_code::ForeignCallRelocation,
    callback: &machine_code::CallbackAddressMaterialization,
    signature: &calling_conventions::CallSignature,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidCallbackAddressCustody {
        caller,
        owner: call.owner,
    };
    let CallSiteOwner::Operation(operation) = call.owner else {
        return Err(invalid());
    };
    let pointer_size = u16::try_from(target.pointer_size).map_err(|_| invalid())?;
    let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| invalid())?;
    let application = &callback.target.application;
    let ordinal = usize::try_from(application.native_ordinal).map_err(|_| invalid())?;
    let nominal_destination = calling_conventions::NativePlace::Parameter(application.parameter);
    let expected_placement = match callback.destination {
        machine_code::CallbackAddressDestination::Register(register) => {
            calling_conventions::ValuePlacement {
                shape: application.shape,
                locations: vec![calling_conventions::ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: application.shape.byte_size,
                }],
            }
        }
        machine_code::CallbackAddressDestination::OutgoingStack { byte_offset } => {
            calling_conventions::ValuePlacement {
                shape: application.shape,
                locations: vec![calling_conventions::ValueLocation::Stack {
                    stack_byte_offset: byte_offset,
                    value_byte_offset: 0,
                    byte_size: application.shape.byte_size,
                    alignment: application.shape.alignment,
                }],
            }
        }
    };
    let context = &callback.target.registrar_context;
    if callback.target.terminal_operation != operation
        || callback
            .target
            .callback_function
            .callback_thunk_placement_index()
            != Some(callback.target.placement_index)
        || callback.target.registrar_application_commitment == [0; 32]
        || application.shape
            != calling_conventions::ValueShape::integer(pointer_size, pointer_alignment)
        || application.placement != expected_placement
        || call.call_plan.parameters.get(ordinal) != Some(&application.placement)
        || callback.target.registrar_boundary_entry_plan.call != call.call_plan
        || call.call_plan.callback_materializations.len() != 1
        || call.call_plan.callback_materializations[0].destination != nominal_destination
        || context.binders.len() != 1
        || context.demands.len() != 1
        || context.demands[0].destination != nominal_destination
    {
        return Err(invalid());
    }
    let validated =
        calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
            callback.target.registrar_boundary_entry_plan.clone(),
            signature,
            context,
        )
        .map_err(|_| invalid())?;
    if validated.plan() != &callback.target.registrar_boundary_entry_plan {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn validate_callback_address_bytes(
    target: NativeTarget,
    caller: MachineId,
    owner: CallSiteOwner,
    function: &machine_code::MachineCodeFunction,
    callback: &machine_code::CallbackAddressMaterialization,
) -> Result<usize, ObjectError> {
    let invalid = || ObjectError::InvalidCallbackAddressCustody { caller, owner };
    let mut expected = Vec::new();
    match (target.architecture, callback.destination, callback.encoding) {
        (
            Architecture::X86_64,
            machine_code::CallbackAddressDestination::Register(register),
            machine_code::CallbackAddressEncoding::X86_64Relative32 { relocation_offset },
        ) => {
            let register =
                crate::object_artifact::replay::instruction_loads::x86_terminal_register(register)
                    .filter(|register| *register != 4)
                    .ok_or_else(invalid)?;
            expected.extend_from_slice(&[
                0x48 | (((register >> 3) & 1) << 2),
                0x8d,
                0x05 | ((register & 7) << 3),
            ]);
            if relocation_offset
                != callback
                    .code_offset
                    .checked_add(expected.len())
                    .ok_or_else(invalid)?
            {
                return Err(invalid());
            }
            expected.extend_from_slice(&[0; 4]);
        }
        (
            Architecture::X86_64,
            machine_code::CallbackAddressDestination::OutgoingStack { byte_offset },
            machine_code::CallbackAddressEncoding::X86_64Relative32 { relocation_offset },
        ) => {
            expected.extend_from_slice(&[0x4c, 0x8d, 0x1d]);
            if relocation_offset
                != callback
                    .code_offset
                    .checked_add(expected.len())
                    .ok_or_else(invalid)?
            {
                return Err(invalid());
            }
            expected.extend_from_slice(&[0; 4]);
            crate::object_artifact::replay::unit::scalar_call_custody::expected_x86_stack_store(
                &mut expected,
                11,
                byte_offset,
            );
        }
        (
            Architecture::Aarch64,
            machine_code::CallbackAddressDestination::Register(register),
            machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                page_relocation_offset,
                page_offset_relocation_offset,
            },
        ) => {
            let register =
                crate::object_artifact::replay::instruction_loads::aarch64_terminal_register(
                    register,
                )
                .ok_or_else(invalid)?;
            if page_relocation_offset != callback.code_offset
                || page_offset_relocation_offset
                    != callback.code_offset.checked_add(4).ok_or_else(invalid)?
            {
                return Err(invalid());
            }
            expected.extend_from_slice(&(0x9000_0000 | u32::from(register)).to_le_bytes());
            expected.extend_from_slice(
                &(0x9100_0000 | (u32::from(register) << 5) | u32::from(register)).to_le_bytes(),
            );
        }
        (
            Architecture::Aarch64,
            machine_code::CallbackAddressDestination::OutgoingStack { byte_offset },
            machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                page_relocation_offset,
                page_offset_relocation_offset,
            },
        ) => {
            if page_relocation_offset != callback.code_offset
                || page_offset_relocation_offset
                    != callback.code_offset.checked_add(4).ok_or_else(invalid)?
            {
                return Err(invalid());
            }
            expected.extend_from_slice(&0x9000_0009u32.to_le_bytes());
            expected.extend_from_slice(&0x9100_0129u32.to_le_bytes());
            expected.extend_from_slice(
                &crate::object_artifact::replay::unit::scalar_call_custody::expected_aarch64_stack_store(9, byte_offset)
                    .ok_or_else(invalid)?
                    .to_le_bytes(),
            );
        }
        _ => return Err(invalid()),
    }
    let end = callback
        .code_offset
        .checked_add(callback.byte_count)
        .ok_or_else(invalid)?;
    if callback.byte_count != expected.len()
        || function.bytes.get(callback.code_offset..end) != Some(expected.as_slice())
    {
        return Err(invalid());
    }
    Ok(end)
}

pub(crate) fn validate_foreign_scalar_arguments(
    target: NativeTarget,
    function: &machine_code::MachineCodeFunction,
    call: &machine_code::ForeignCallRelocation,
) -> Result<(), ObjectError> {
    let caller = function.machine;
    let invalid = || ObjectError::InvalidForeignCallArgument {
        caller,
        owner: call.owner,
    };
    let shapes = call
        .scalar_arguments
        .iter()
        .map(|argument| {
            let semantic_vocabulary::ScalarType::Integer(scalar_type) =
                argument.source.scalar_type()
            else {
                return Err(invalid());
            };
            let bits = scalar_type.bits();
            if scalar_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                || !matches!(bits, 8 | 16 | 32 | 64)
                || matches!(
                    argument.source,
                    machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                        value,
                        ..
                    } if !scalar_type.admits(value)
                )
            {
                return Err(invalid());
            }
            let byte_size = bits / 8;
            Ok(calling_conventions::ValueShape::integer(
                byte_size, byte_size,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result_shape = call
        .scalar_result
        .as_ref()
        .map(|result| {
            let semantic_vocabulary::ScalarType::Integer(result_type) = result.home.scalar_type
            else {
                return Err(invalid());
            };
            let expected_shape =
                crate::object_artifact::replay::unit::scalar_call_custody::integer_shape(
                    result_type,
                )
                .ok_or_else(invalid)?;
            if result.home.defining_operation
                != match call.owner {
                    CallSiteOwner::Operation(operation) => operation,
                    CallSiteOwner::CleanupAction { .. } => return Err(invalid()),
                }
                || result.home.shape != expected_shape
                || !function.unit_scalar_homes.contains(&result.home)
            {
                return Err(invalid());
            }
            Ok(expected_shape)
        })
        .transpose()?;
    let callback_ordinal = call
        .callback_address
        .as_ref()
        .map(|callback| usize::try_from(callback.target.application.native_ordinal))
        .transpose()
        .map_err(|_| invalid())?;
    if callback_ordinal.is_some_and(|ordinal| ordinal > shapes.len()) {
        return Err(invalid());
    }
    let native_parameter_count = shapes.len() + usize::from(callback_ordinal.is_some());
    let mut native_shapes = Vec::with_capacity(native_parameter_count);
    let mut scalar_shape_index = 0usize;
    for native_index in 0..native_parameter_count {
        if callback_ordinal == Some(native_index) {
            native_shapes.push(
                call.callback_address
                    .as_ref()
                    .expect("callback ordinal has callback custody")
                    .target
                    .application
                    .shape,
            );
        } else {
            native_shapes.push(*shapes.get(scalar_shape_index).ok_or_else(invalid)?);
            scalar_shape_index += 1;
        }
    }
    if scalar_shape_index != shapes.len() {
        return Err(invalid());
    }
    let signature = calling_conventions::CallSignature {
        parameters: native_shapes,
        result: result_shape,
    };
    let expected_boundary = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        calling_conventions::CallingPolicy::native_for_target(target),
        &signature,
    )
    .map_err(|_| invalid())?;
    let expected_plan = &expected_boundary.plan().call;
    let mut ordinary_call_plan = call.call_plan.clone();
    ordinary_call_plan.callback_materializations.clear();
    if call.boundary_entry_plan.call != call.call_plan
        || call.boundary_entry_plan.state != expected_boundary.plan().state
        || ordinary_call_plan != *expected_plan
        || call.call_plan.policy != calling_conventions::CallingPolicy::native_for_target(target)
        || call.call_plan.entry_control != calling_conventions::EntryControl::CallReturn
        || call.call_plan.parameters.len() != native_parameter_count
    {
        return Err(invalid());
    }
    match &call.callback_address {
        Some(callback) => {
            validate_callback_plan_custody(target, caller, call, callback, &signature)?
        }
        None if call.call_plan.callback_materializations.is_empty() => {}
        None => return Err(invalid()),
    }
    let expected_outbound =
        expected_foreign_scalar_outbound_bytes(expected_plan, target.architecture)
            .ok_or_else(invalid)?;
    match (expected_outbound, call.unit_stack.outbound) {
        (0, None) => {}
        (expected, Some(outbound)) if outbound.byte_size == expected => {}
        _ => return Err(invalid()),
    }
    let call_start = match target.architecture {
        Architecture::X86_64 => call.offset.saturating_sub(1),
        Architecture::Aarch64 => call.offset,
    };
    let CallSiteOwner::Operation(operation) = call.owner else {
        return Err(invalid());
    };
    let call_end = call.offset.checked_add(4).ok_or_else(invalid)?;
    let operation_start = match target.architecture {
        Architecture::X86_64 => call.x86_floating_control.map(|control| control.save_offset),
        Architecture::Aarch64 => call
            .aarch64_floating_control
            .map(|control| control.save_offset),
    }
    .unwrap_or_else(|| {
        call.unit_stack.outbound.map_or_else(
            || {
                call.scalar_arguments
                    .first()
                    .map(|argument| argument.code_offset)
                    .into_iter()
                    .chain(
                        call.callback_address
                            .as_ref()
                            .map(|callback| callback.code_offset),
                    )
                    .min()
                    .unwrap_or(call_start)
            },
            |outbound| outbound.allocation_offset,
        )
    });
    let post_save = match target.architecture {
        Architecture::X86_64 => call.x86_floating_control.map(|control| {
            control
                .save_offset
                .checked_add(control.save_byte_count)
                .ok_or_else(invalid)
        }),
        Architecture::Aarch64 => call.aarch64_floating_control.map(|control| {
            control
                .save_offset
                .checked_add(control.save_byte_count)
                .ok_or_else(invalid)
        }),
    }
    .transpose()?
    .unwrap_or(operation_start);
    let mut argument_cursor = if let Some(outbound) = call.unit_stack.outbound {
        outbound
            .allocation_offset
            .checked_add(outbound.allocation_byte_count)
            .ok_or_else(invalid)?
    } else {
        post_save
    };
    let post_call = if let Some(outbound) = call.unit_stack.outbound {
        if outbound.release_offset != call_end {
            return Err(invalid());
        }
        outbound
            .release_offset
            .checked_add(outbound.release_byte_count)
            .ok_or_else(invalid)?
    } else {
        call_end
    };
    let post_control = match target.architecture {
        Architecture::X86_64 => call.x86_floating_control.map(|control| {
            control
                .restore_offset
                .checked_add(control.restore_byte_count)
                .ok_or_else(invalid)
        }),
        Architecture::Aarch64 => call.aarch64_floating_control.map(|control| {
            control
                .restore_offset
                .checked_add(control.restore_byte_count)
                .ok_or_else(invalid)
        }),
    }
    .transpose()?
    .unwrap_or(post_call);
    let operation_end = if let Some(result) = &call.scalar_result {
        let expected_shape = result.home.shape;
        if result.code_offset != post_control
            || call.call_plan.result.as_ref() != Some(&result.source)
            || !matches!(
                result.source.locations.as_slice(),
                [calling_conventions::ValueLocation::Register {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                }] if *byte_size == expected_shape.byte_size
            )
        {
            return Err(invalid());
        }
        let expected = crate::object_artifact::replay::unit::scalar_call_custody::expected_unit_scalar_result_bytes(target, result)
            .ok_or_else(invalid)?;
        let result_end = result
            .code_offset
            .checked_add(result.byte_count)
            .ok_or_else(invalid)?;
        if result.byte_count != expected.len()
            || function.bytes.get(result.code_offset..result_end) != Some(expected.as_slice())
        {
            return Err(invalid());
        }
        result_end
    } else {
        if call.call_plan.result.is_some() {
            return Err(invalid());
        }
        post_control
    };
    let attributions = function
        .semantic_code_attribution
        .iter()
        .filter(|row| row.site == SemanticCodeSite::Operation(operation))
        .filter(|row| row.operation_ordinal == call.operation_ordinal)
        .filter(|row| {
            row.code_offset == operation_start
                && row
                    .code_offset
                    .checked_add(row.byte_count)
                    .is_some_and(|end| end == operation_end)
        })
        .collect::<Vec<_>>();
    let [attribution] = attributions.as_slice() else {
        return Err(invalid());
    };

    let mut scalar_index = 0usize;
    for native_index in 0..call.call_plan.parameters.len() {
        if callback_ordinal == Some(native_index) {
            let callback = call.callback_address.as_ref().ok_or_else(invalid)?;
            if callback.code_offset != argument_cursor {
                return Err(invalid());
            }
            argument_cursor =
                validate_callback_address_bytes(target, caller, call.owner, function, callback)?;
            continue;
        }
        let argument = call
            .scalar_arguments
            .get(scalar_index)
            .ok_or_else(invalid)?;
        let shape = shapes.get(scalar_index).ok_or_else(invalid)?;
        let expected_placement = call
            .call_plan
            .parameters
            .get(native_index)
            .ok_or_else(invalid)?;
        let placed_bytes = match argument.placement.locations.as_slice() {
            [
                calling_conventions::ValueLocation::Register {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ]
            | [
                calling_conventions::ValueLocation::Stack {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] => *byte_size,
            _ => return Err(invalid()),
        };
        if argument.parameter_index != native_index as u32
            || argument.placement != *expected_placement
            || argument.placement.shape != *shape
            || placed_bytes != shape.byte_size
            || argument.code_offset != argument_cursor
        {
            return Err(invalid());
        }
        validate_foreign_scalar_source(function, attribution, argument)?;
        let expected_bytes =
            expected_foreign_scalar_argument_bytes(target, argument, expected_outbound)
                .ok_or_else(invalid)?;
        let argument_end = argument
            .code_offset
            .checked_add(argument.byte_count)
            .ok_or_else(invalid)?;
        if argument.byte_count != expected_bytes.len()
            || function.bytes.get(argument.code_offset..argument_end)
                != Some(expected_bytes.as_slice())
        {
            return Err(invalid());
        }
        argument_cursor = argument_end;
        scalar_index += 1;
    }
    if scalar_index != call.scalar_arguments.len() || argument_cursor != call_start {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn expected_foreign_scalar_outbound_bytes(
    call_plan: &calling_conventions::CallPlan,
    architecture: Architecture,
) -> Option<u32> {
    let mut extent = u32::from(call_plan.shadow_bytes);
    for placement in &call_plan.parameters {
        for location in &placement.locations {
            if let calling_conventions::ValueLocation::Stack {
                stack_byte_offset,
                byte_size,
                ..
            } = location
            {
                let slot_bytes = u32::from(*byte_size).max(8);
                extent = extent.max(stack_byte_offset.checked_add(slot_bytes)?);
            }
        }
    }
    match architecture {
        Architecture::X86_64 => {
            let padding = (8 + 16 - (extent % 16)) % 16;
            extent.checked_add(padding)
        }
        Architecture::Aarch64 => {
            let padding = (16 - (extent % 16)) % 16;
            extent.checked_add(padding)
        }
    }
}

pub(crate) fn validate_foreign_scalar_source(
    function: &machine_code::MachineCodeFunction,
    attribution: &SemanticCodeAttribution,
    argument: &machine_code::ForeignCallScalarArgumentRecord,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidForeignCallArgument {
        caller: function.machine,
        owner: CallSiteOwner::Operation(match attribution.site {
            SemanticCodeSite::Operation(operation) => operation,
            SemanticCodeSite::Edge(_) => unreachable!("foreign call attribution is an operation"),
        }),
    };
    let exact_sources = match argument.source {
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedBoundary { .. }
        | machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { .. }
        | machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall { .. }
        | machine_code::InternalUnitScalarArgumentSourceRecord::Parameter { .. } => {
            return Err(invalid());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => function
            .unit_integer_constants
            .iter()
            .filter(|constant| {
                constant.defining_operation == defining_operation
                    && constant.source_value == source_value
                    && constant.scalar_type == scalar_type
                    && constant.value == value
                    && constant.operation_ordinal < attribution.operation_ordinal
            })
            .count(),
        machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate { .. } => {
            return Err(invalid());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) => {
            if !function.unit_scalar_homes.contains(&home) {
                return Err(invalid());
            }
            crate::object_artifact::replay::unit::scalar_call_custody::exact_preceding_unit_scalar_home_producer_count(
                function,
                home,
                attribution.operation_ordinal,
                argument.code_offset,
            )
        }
    };
    (exact_sources == 1).then_some(()).ok_or_else(invalid)
}

pub(crate) fn expected_foreign_scalar_argument_bytes(
    target: NativeTarget,
    argument: &machine_code::ForeignCallScalarArgumentRecord,
    outbound_bytes: u32,
) -> Option<Vec<u8>> {
    let (register, stack) = match argument.placement.locations.as_slice() {
        [calling_conventions::ValueLocation::Register { register, .. }] => (
            Some(match target.architecture {
                Architecture::X86_64 => {
                    crate::object_artifact::replay::instruction_loads::x86_terminal_register(
                        *register,
                    )?
                }
                Architecture::Aarch64 => {
                    crate::object_artifact::replay::instruction_loads::aarch64_terminal_register(
                        *register,
                    )?
                }
            }),
            None,
        ),
        [
            calling_conventions::ValueLocation::Stack {
                stack_byte_offset, ..
            },
        ] => (None, Some(*stack_byte_offset)),
        _ => return None,
    };
    let register = match target.architecture {
        Architecture::X86_64 => register.unwrap_or(11),
        Architecture::Aarch64 => register.unwrap_or(9),
    };
    let mut bytes = Vec::new();
    match argument.source {
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedBoundary { .. }
        | machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { .. }
        | machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall { .. }
        | machine_code::InternalUnitScalarArgumentSourceRecord::Parameter { .. } => {
            return None;
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            scalar_type,
            value,
            ..
        } => {
            let bits = scalar_type.bits();
            let mask = if bits == 64 {
                u64::MAX
            } else {
                (1_u64 << bits) - 1
            };
            let value_bits = match (scalar_type.sign(), value) {
                (
                    semantic_vocabulary::IntegerSign::Signed,
                    semantic_vocabulary::IntegerValue::Signed(value),
                ) => value as u128 as u64,
                (
                    semantic_vocabulary::IntegerSign::Unsigned,
                    semantic_vocabulary::IntegerValue::Unsigned(value),
                ) => value as u64,
                _ => return None,
            } & mask;
            match target.architecture {
                Architecture::X86_64 if bits <= 32 => {
                    if register >= 8 {
                        bytes.push(0x41);
                    }
                    bytes.push(0xb8 | (register & 7));
                    bytes.extend_from_slice(&(value_bits as u32).to_le_bytes());
                }
                Architecture::X86_64 => {
                    bytes.extend_from_slice(&[0x48 | ((register >> 3) & 1), 0xb8 | (register & 7)]);
                    bytes.extend_from_slice(&value_bits.to_le_bytes());
                }
                Architecture::Aarch64 => {
                    for chunk in 0..4 {
                        let immediate = ((value_bits >> (chunk * 16)) & 0xffff) as u32;
                        if chunk == 0 || immediate != 0 {
                            let base = if chunk == 0 { 0xd280_0000 } else { 0xf280_0000 };
                            let instruction = base
                                | ((chunk as u32) << 21)
                                | (immediate << 5)
                                | u32::from(register);
                            bytes.extend_from_slice(&instruction.to_le_bytes());
                        }
                    }
                }
            }
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate { .. } => {
            return None;
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) => {
            match target.architecture {
                Architecture::X86_64 => {
                    crate::object_artifact::replay::instruction_loads::expected_x86_stack_load(
                        &mut bytes,
                        register,
                        outbound_bytes.checked_add(home.byte_offset)?,
                        home.shape.byte_size,
                    )?;
                }
                Architecture::Aarch64 => {
                    let instruction = crate::object_artifact::replay::instruction_loads::expected_aarch64_stack_load(
                        register,
                        outbound_bytes.checked_add(home.byte_offset)?,
                        home.shape.byte_size,
                    )?;
                    bytes.extend_from_slice(&instruction.to_le_bytes());
                }
            }
        }
    }
    if let Some(offset) = stack {
        match target.architecture {
            Architecture::X86_64 => {
                crate::object_artifact::replay::unit::scalar_call_custody::expected_x86_stack_store(
                    &mut bytes, register, offset,
                );
            }
            Architecture::Aarch64 => {
                let instruction =
                    crate::object_artifact::replay::unit::scalar_call_custody::expected_aarch64_stack_store(register, offset)?;
                bytes.extend_from_slice(&instruction.to_le_bytes());
            }
        }
    }
    Some(bytes)
}
