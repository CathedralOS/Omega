//! Independent replay for forwarded existential descriptor adapter tables.

use omega_calling_conventions::{
    CallSignature, CallingPolicy, IndirectPointerLocation, ValueLocation, ValueShape,
    evaluate_call_plan,
};
use omega_machine_code::{
    DynamicParameterCallMechanismRecord, DynamicTableAddressEncoding,
    ForwardedDynamicDescriptorAdapterRecord, MachineCodeFunction, SemanticCodeSite,
};
use omega_target::{Architecture, NativeTarget};
use psi_core::MachineId;

use super::instruction_loads::{aarch64_terminal_register, x86_terminal_register};
use super::{ObjectError, same_dynamic_table_application};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ValidatedForwardedDynamicApplication {
    pub application: psi_terminal::ClosedConformanceApplication,
    pub adapters: Vec<ForwardedDynamicDescriptorAdapterRecord>,
}

pub(super) fn validate_forwarded_dynamic_descriptors(
    target: NativeTarget,
    functions: &[MachineCodeFunction],
) -> Result<Vec<ValidatedForwardedDynamicApplication>, ObjectError> {
    let machine_functions = functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut applications = Vec::<ValidatedForwardedDynamicApplication>::new();

    for function in functions {
        validate_parameter_dispatches(target, function)?;
        for call in &function.forwarded_dynamic_descriptor_calls {
            let invalid = || ObjectError::InvalidForwardedDynamicDescriptorEvidence {
                caller: function.machine,
                operation: call.psi_operation,
            };
            let operation_end = call
                .code_offset
                .checked_add(call.byte_count)
                .ok_or_else(invalid)?;
            if operation_end > function.bytes.len()
                || !function.provenance.operations.contains(&call.psi_operation)
                || function
                    .semantic_code_attribution
                    .iter()
                    .filter(|attribution| {
                        attribution.site == SemanticCodeSite::Operation(call.psi_operation)
                            && attribution.operation_ordinal == call.operation_ordinal
                            && attribution.code_offset == call.code_offset
                            && attribution.byte_count == call.byte_count
                    })
                    .count()
                    != 1
                || call.dynamic_arguments.len() != 1
                || call.call_plan.parameters.len() != 2
                || machine_functions.get(&call.callee).is_none()
                || call.semantic_result.is_some() != call.result.is_some()
            {
                return Err(invalid());
            }
            validate_call_result(target, function, call, operation_end).ok_or_else(invalid)?;
            let argument = &call.dynamic_arguments[0];
            if !argument.custody.has_complete_custody(
                function.machine,
                call.psi_operation,
                call.callee,
            ) {
                return Err(invalid());
            }
            let omega_abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                application,
                rebound,
                ..
            } = &argument.custody.source
            else {
                return Err(invalid());
            };
            if argument.instance.place != rebound.source.place
                || argument.instance.access != rebound.source.access
                || argument.instance.path != rebound.source.path
                || argument.adapters.len() != application.rows.len()
            {
                return Err(invalid());
            }
            validate_table_address(target, function, argument).ok_or_else(invalid)?;
            for (row_index, (row, adapter)) in
                application.rows.iter().zip(&argument.adapters).enumerate()
            {
                validate_adapter(
                    target,
                    application,
                    row,
                    row_index,
                    adapter,
                    argument,
                    &machine_functions,
                )
                .ok_or_else(invalid)?;
            }
            if let Some(existing) = applications
                .iter()
                .find(|existing| existing.application.commitment == application.commitment)
            {
                if !same_dynamic_table_application(&existing.application, application)
                    || existing.adapters != argument.adapters
                {
                    return Err(ObjectError::ForwardedDynamicDescriptorCommitmentCollision);
                }
            } else {
                applications.push(ValidatedForwardedDynamicApplication {
                    application: application.clone(),
                    adapters: argument.adapters.clone(),
                });
            }
        }
    }
    Ok(applications)
}

fn validate_call_result(
    target: NativeTarget,
    function: &MachineCodeFunction,
    call: &omega_machine_code::ForwardedDynamicDescriptorCallRecord,
    operation_end: usize,
) -> Option<()> {
    let direct_call_end = call
        .direct_call_offset
        .checked_add(call.direct_call_byte_count)?;
    let post_call_offset = if let Some(outbound) = call.unit_stack.outbound {
        let allocation_end = outbound
            .allocation_offset
            .checked_add(outbound.allocation_byte_count)?;
        if allocation_end > call.direct_call_offset || outbound.release_offset != direct_call_end {
            return None;
        }
        outbound
            .release_offset
            .checked_add(outbound.release_byte_count)?
    } else {
        direct_call_end
    };
    let result_shape = call
        .semantic_result
        .and_then(|result| super::unit_scalar_call_custody::scalar_home_shape(result.scalar_type));
    let pointer = ValueShape::integer(
        u16::try_from(target.pointer_size).ok()?,
        u16::try_from(target.pointer_alignment).ok()?,
    );
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![pointer; 2],
            result: result_shape,
        },
    )
    .ok()?;
    if expected_call_plan != call.call_plan {
        return None;
    }
    let (Some(semantic_result), Some(result), Some(result_shape)) =
        (call.semantic_result, call.result.as_ref(), result_shape)
    else {
        return (call.semantic_result.is_none()
            && call.result.is_none()
            && call.call_plan.result.is_none()
            && post_call_offset == operation_end)
            .then_some(());
    };
    let expected_bytes =
        super::unit_scalar_call_custody::expected_unit_scalar_result_bytes(target, result)?;
    let result_end = result.code_offset.checked_add(result.byte_count)?;
    (call.call_plan == expected_call_plan
        && call.call_plan.result.as_ref() == Some(&result.source)
        && result.home.defining_operation == call.psi_operation
        && result.home.source_value == semantic_result.value
        && result.home.scalar_type == semantic_result.scalar_type
        && result.home.shape == result_shape
        && function.unit_scalar_homes.contains(&result.home)
        && result.code_offset == post_call_offset
        && result.byte_count == expected_bytes.len()
        && result_end == operation_end
        && function.bytes.get(result.code_offset..result_end) == Some(expected_bytes.as_slice()))
    .then_some(())
}

fn validate_parameter_dispatches(
    target: NativeTarget,
    function: &MachineCodeFunction,
) -> Result<(), ObjectError> {
    for call in &function.dynamic_parameter_calls {
        let invalid = || ObjectError::InvalidDynamicParameterCallEvidence {
            caller: function.machine,
            operation: call.psi_operation,
        };
        let end = call
            .code_offset
            .checked_add(call.byte_count)
            .ok_or_else(invalid)?;
        let call_end = call
            .indirect_call_offset
            .checked_add(call.indirect_call_byte_count)
            .ok_or_else(invalid)?;
        let expected_result_shape = match (call.requirement.result, call.scalar_type) {
            (psi_terminal::ClosedConformanceCallableResult::Unit, None) => None,
            (
                psi_terminal::ClosedConformanceCallableResult::I32,
                Some(psi_core::ScalarType::Integer(integer)),
            ) if integer.sign() == psi_core::IntegerSign::Signed && integer.bits() == 32 => {
                Some(ValueShape::integer(4, 4))
            }
            (
                psi_terminal::ClosedConformanceCallableResult::Bool,
                Some(psi_core::ScalarType::Boolean),
            ) => Some(ValueShape::integer(1, 1)),
            _ => return Err(invalid()),
        };
        let pointer = ValueShape::integer(
            u16::try_from(target.pointer_size).map_err(|_| invalid())?,
            u16::try_from(target.pointer_alignment).map_err(|_| invalid())?,
        );
        let expected_function_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![pointer, pointer],
                result: expected_result_shape,
            },
        )
        .map_err(|_| invalid())?;
        let expected_dispatch_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![pointer],
                result: expected_result_shape,
            },
        )
        .map_err(|_| invalid())?;
        if end > function.bytes.len()
            || call_end > end
            || !function.provenance.operations.contains(&call.psi_operation)
            || call.table_slot_byte_offset
                != call.requirement.slot.checked_mul(8).ok_or_else(invalid)?
            || call.parameter.owner != function.machine
            || call.parameter.trait_identity != call.requirement.declaring_trait_identity
            || call.source_value.is_some() != call.scalar_type.is_some()
            || call.function_call_plan != expected_function_plan
            || call.dispatch_call_plan != expected_dispatch_plan
            || call
                .parameter
                .requirements
                .get(usize::try_from(call.requirement.slot).map_err(|_| invalid())?)
                != Some(&call.requirement)
        {
            return Err(invalid());
        }
        let bytes = function
            .bytes
            .get(call.indirect_call_offset..call_end)
            .ok_or_else(invalid)?;
        let valid_call = match (target.architecture, call.mechanism) {
            (
                Architecture::X86_64,
                DynamicParameterCallMechanismRecord::X86MemoryIndirect { table },
            ) => {
                let register = x86_terminal_register(table).ok_or_else(invalid)?;
                let mut expected = Vec::new();
                if register >= 8 {
                    expected.push(0x41);
                }
                expected.push(0xff);
                if register & 7 == 4 {
                    expected.extend_from_slice(&[0x94, 0x24]);
                } else {
                    expected.push(0x90 | (register & 7));
                }
                expected.extend_from_slice(&call.table_slot_byte_offset.to_le_bytes());
                bytes == expected
            }
            (
                Architecture::Aarch64,
                DynamicParameterCallMechanismRecord::Aarch64LoadedIndirect { target, .. },
            ) => {
                let register = aarch64_terminal_register(target).ok_or_else(invalid)?;
                call.indirect_call_byte_count >= 4
                    && bytes.ends_with(&(0xd63f_0000 | (u32::from(register) << 5)).to_le_bytes())
            }
            _ => false,
        };
        if !valid_call {
            return Err(invalid());
        }
    }
    Ok(())
}

fn validate_table_address(
    target: NativeTarget,
    function: &MachineCodeFunction,
    argument: &omega_machine_code::ForwardedDynamicDescriptorArgumentRecord,
) -> Option<()> {
    let materialization = &argument.table_address;
    let end = materialization
        .code_offset
        .checked_add(materialization.byte_count)?;
    let bytes = function.bytes.get(materialization.code_offset..end)?;
    match (target.architecture, materialization.encoding) {
        (
            Architecture::X86_64,
            DynamicTableAddressEncoding::X86_64Relative32 { relocation_offset },
        ) => {
            let register = x86_terminal_register(argument.table_destination)?;
            let expected = [
                0x48 | (((register >> 3) & 1) << 2),
                0x8d,
                0x05 | ((register & 7) << 3),
                0,
                0,
                0,
                0,
            ];
            (relocation_offset == materialization.code_offset + 3 && bytes == expected)
                .then_some(())
        }
        (
            Architecture::Aarch64,
            DynamicTableAddressEncoding::Aarch64PageAddress {
                page_relocation_offset,
                page_offset_relocation_offset,
            },
        ) => {
            let register = aarch64_terminal_register(argument.table_destination)?;
            let expected = [
                (0x9000_0000 | u32::from(register)).to_le_bytes(),
                (0x9100_0000 | (u32::from(register) << 5) | u32::from(register)).to_le_bytes(),
            ]
            .concat();
            (page_relocation_offset == materialization.code_offset
                && page_offset_relocation_offset == materialization.code_offset + 4
                && bytes == expected)
                .then_some(())
        }
        _ => None,
    }
}

fn validate_adapter(
    target: NativeTarget,
    application: &psi_terminal::ClosedConformanceApplication,
    row: &psi_terminal::ClosedConformanceRow,
    row_index: usize,
    adapter: &ForwardedDynamicDescriptorAdapterRecord,
    argument: &omega_machine_code::ForwardedDynamicDescriptorArgumentRecord,
    machine_functions: &std::collections::BTreeMap<MachineId, &MachineCodeFunction>,
) -> Option<()> {
    let callable_identity = row.realization_callable_identity.as_deref()?;
    let callable = application
        .realization_callables
        .iter()
        .find(|callable| callable.source_callable_identity == callable_identity)?;
    machine_functions.get(&callable.machine)?;
    let pointer = ValueShape::integer(
        u16::try_from(target.pointer_size).ok()?,
        u16::try_from(target.pointer_alignment).ok()?,
    );
    let result = match callable.result {
        psi_terminal::ClosedConformanceCallableResult::Unit => None,
        psi_terminal::ClosedConformanceCallableResult::I32 => Some(ValueShape::integer(4, 4)),
        psi_terminal::ClosedConformanceCallableResult::Bool => Some(ValueShape::integer(1, 1)),
    };
    let erased = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![pointer],
            result,
        },
    )
    .ok()?;
    let realization_parameter_shape =
        if argument.instance.access == psi_terminal::StructuralAccess::MutableBorrow {
            ValueShape::borrowed_reference(
                argument.instance.shape.byte_size,
                argument.instance.shape.alignment,
            )
        } else {
            argument.instance.shape
        };
    let realization = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![realization_parameter_shape],
            result,
        },
    )
    .ok()?;
    if adapter.identity.application != application.commitment
        || usize::try_from(adapter.identity.row_index).ok()? != row_index
        || adapter.identity.realization != callable.machine
        || adapter.requirement_identity != row.requirement_identity
        || adapter.realization_identity != row.realization_identity
        || adapter.realization_callable_identity != callable_identity
        || adapter.result != callable.result
        || adapter.source_shape != argument.instance.shape
        || adapter.erased_call_plan != erased
        || adapter.realization_call_plan != realization
    {
        return None;
    }
    let expected = expected_adapter_bytes(target, &erased, &realization, adapter.source_shape)?;
    (adapter.bytes == expected
        && adapter
            .argument_code_offset
            .checked_add(adapter.argument_byte_count)?
            <= adapter.direct_call_offset
        && adapter
            .direct_call_offset
            .checked_add(adapter.direct_call_byte_count)?
            <= adapter.return_offset
        && adapter
            .return_offset
            .checked_add(adapter.return_byte_count)?
            == adapter.bytes.len())
    .then_some(())
}

fn expected_adapter_bytes(
    target: NativeTarget,
    erased: &omega_calling_conventions::CallPlan,
    realization: &omega_calling_conventions::CallPlan,
    source_shape: ValueShape,
) -> Option<Vec<u8>> {
    let [erased_parameter] = erased.parameters.as_slice() else {
        return None;
    };
    let [realization_parameter] = realization.parameters.as_slice() else {
        return None;
    };
    match target.architecture {
        Architecture::X86_64 => {
            let erased_register =
                exact_register(erased_parameter).and_then(x86_terminal_register)?;
            let mut bytes = Vec::new();
            x86_argument(
                &mut bytes,
                erased_register,
                realization_parameter,
                source_shape,
            )?;
            let shadow = u32::from(realization.shadow_bytes);
            let stack = shadow.checked_add((8 + 16 - shadow % 16) % 16)?;
            if stack != 0 {
                x86_adjust_sp(&mut bytes, stack, false);
            }
            bytes.push(0xe8);
            bytes.extend_from_slice(&0_i32.to_le_bytes());
            if stack != 0 {
                x86_adjust_sp(&mut bytes, stack, true);
            }
            bytes.push(0xc3);
            Some(bytes)
        }
        Architecture::Aarch64 => {
            let erased_register =
                exact_register(erased_parameter).and_then(aarch64_terminal_register)?;
            let mut words = vec![0xd100_03ff | (16 << 10), 0xf900_03fe];
            aarch64_argument(
                &mut words,
                erased_register,
                realization_parameter,
                source_shape,
            )?;
            words.extend([
                0x9400_0000,
                0xf940_03fe,
                0x9100_03ff | (16 << 10),
                0xd65f_03c0,
            ]);
            Some(words.into_iter().flat_map(u32::to_le_bytes).collect())
        }
    }
}

fn exact_register(
    placement: &omega_calling_conventions::ValuePlacement,
) -> Option<omega_target_operations::MachineRegister> {
    let [
        ValueLocation::Register {
            register,
            value_byte_offset: 0,
            ..
        },
    ] = placement.locations.as_slice()
    else {
        return None;
    };
    Some(*register)
}

fn x86_argument(
    bytes: &mut Vec<u8>,
    source: u8,
    placement: &omega_calling_conventions::ValuePlacement,
    shape: ValueShape,
) -> Option<()> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == shape.byte_size && *byte_size <= 8 => {
            let destination = x86_terminal_register(*register)?;
            x86_memory_load(bytes, destination, source, *byte_size)?;
        }
        [
            ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                copy_stack_byte_offset: None,
                ..
            },
        ] => {
            let destination = x86_terminal_register(*register)?;
            if destination != source {
                bytes.extend_from_slice(&[
                    0x48 | (((source >> 3) & 1) << 2) | ((destination >> 3) & 1),
                    0x89,
                    0xc0 | ((source & 7) << 3) | (destination & 7),
                ]);
            }
        }
        _ => return None,
    }
    Some(())
}

fn x86_memory_load(bytes: &mut Vec<u8>, destination: u8, base: u8, width: u16) -> Option<()> {
    let rex = 0x40 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1);
    match width {
        1 => bytes.extend_from_slice(&[rex, 0x0f, 0xb6]),
        2 => bytes.extend_from_slice(&[0x66, rex, 0x0f, 0xb7]),
        4 => bytes.extend_from_slice(&[rex, 0x8b]),
        8 => bytes.extend_from_slice(&[rex | 0x08, 0x8b]),
        _ => return None,
    }
    ((base & 7) != 5).then(|| bytes.push(((destination & 7) << 3) | (base & 7)))
}

fn x86_adjust_sp(bytes: &mut Vec<u8>, size: u32, add: bool) {
    if size <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x48, 0x83, if add { 0xc4 } else { 0xec }, size as u8]);
    } else {
        bytes.extend_from_slice(&[0x48, 0x81, if add { 0xc4 } else { 0xec }]);
        bytes.extend_from_slice(&size.to_le_bytes());
    }
}

fn aarch64_argument(
    words: &mut Vec<u32>,
    source: u8,
    placement: &omega_calling_conventions::ValuePlacement,
    shape: ValueShape,
) -> Option<()> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == shape.byte_size && matches!(*byte_size, 1 | 2 | 4 | 8) => {
            let destination = aarch64_terminal_register(*register)?;
            let base = match *byte_size {
                1 => 0x3940_0000,
                2 => 0x7940_0000,
                4 => 0xb940_0000,
                8 => 0xf940_0000,
                _ => return None,
            };
            words.push(base | (u32::from(source) << 5) | u32::from(destination));
        }
        [
            ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                copy_stack_byte_offset: None,
                ..
            },
        ] => {
            let destination = aarch64_terminal_register(*register)?;
            if destination != source {
                words.push(0xaa00_03e0 | (u32::from(source) << 16) | u32::from(destination));
            }
        }
        _ => return None,
    }
    Some(())
}
