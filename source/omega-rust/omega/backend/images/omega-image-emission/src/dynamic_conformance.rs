//! Exact replay of rebound named-dynamic descriptors and indirect calls.

use omega_calling_conventions::ValueLocation;
use omega_machine_code::{
    DynamicCallRecord, DynamicInstanceMaterializationRecord, DynamicTableAddressEncoding,
    MachineCodeFunction, SemanticCodeSite, StoredDynamicCallRecord,
};
use omega_target::{Architecture, NativeTarget};
use psi_core::MachineId;

use super::instruction_loads::{aarch64_terminal_register, x86_terminal_register};
use super::unit_scalar_call_custody::expected_unit_scalar_result_bytes;
use super::unit_stack::validate_stack_adjustment_pair;
use super::{ObjectError, ObjectUnitStack};

pub(super) fn validate_dynamic_calls(
    target: NativeTarget,
    function: &MachineCodeFunction,
    machine_functions: &std::collections::BTreeMap<MachineId, &MachineCodeFunction>,
    function_stack: Option<&ObjectUnitStack>,
) -> Result<u32, ObjectError> {
    let mut previous_end = None;
    let mut operations = std::collections::BTreeSet::new();
    let mut peak = function_stack.map_or(0, |stack| stack.frame_bytes);
    for call in &function.dynamic_calls {
        let invalid = || ObjectError::InvalidDynamicCallEvidence {
            caller: function.machine,
            operation: call.psi_operation,
        };
        let operation_end = call
            .code_offset
            .checked_add(call.byte_count)
            .ok_or_else(invalid)?;
        if previous_end.is_some_and(|end| end > call.code_offset)
            || operation_end > function.bytes.len()
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
            || !call
                .dynamic_dispatch
                .has_complete_application_custody(function.machine, call.psi_operation)
            || !operations.insert(call.psi_operation)
            || call.initial_instance.source.access == psi_terminal::StructuralAccess::Owned
            || call.rebound_instance.source.access == psi_terminal::StructuralAccess::Owned
            || call.descriptor_abi.instance_byte_offset != 0
            || call.descriptor_abi.table_byte_offset != 8
            || call.descriptor_abi.word_byte_size != 8
            || call.descriptor_abi.total_byte_size != 16
            || call.descriptor_abi.byte_alignment != 8
            || call.descriptor_home_byte_offset % 8 != 0
            || call.result.as_ref().is_some_and(|result| {
                result.home.defining_operation != call.psi_operation
                    || call.call_plan.result.as_ref() != Some(&result.source)
                    || !function.unit_scalar_homes.contains(&result.home)
            })
            || call.result.is_none() != call.call_plan.result.is_none()
            || call.call_plan.parameters.as_slice()
                != std::slice::from_ref(&call.argument.destination)
        {
            return Err(invalid());
        }
        let selected_row = call
            .dynamic_dispatch
            .application
            .rows
            .iter()
            .position(|row| {
                row.declaring_trait_identity
                    == call.dynamic_dispatch.dispatch.declaring_trait_identity
                    && row.public_requirement_identity
                        == call.dynamic_dispatch.dispatch.public_requirement_identity
                    && row.requirement_identity
                        == call.dynamic_dispatch.dispatch.requirement_identity
                    && row.realization_identity
                        == call.dynamic_dispatch.dispatch.realization_identity
                    && row.realization_callable_identity.as_deref()
                        == Some(
                            call.dynamic_dispatch
                                .dispatch
                                .realization_callable_identity
                                .as_str(),
                        )
            })
            .ok_or_else(invalid)?;
        let selected_offset = u32::try_from(selected_row)
            .ok()
            .and_then(|row| row.checked_mul(8))
            .ok_or_else(invalid)?;
        let selected_callable = call
            .dynamic_dispatch
            .application
            .realization_callables
            .iter()
            .find(|callable| {
                callable.machine == call.dynamic_dispatch.dispatch.realization
                    && callable.source_callable_identity
                        == call.dynamic_dispatch.dispatch.realization_callable_identity
            })
            .ok_or_else(invalid)?;
        let result_matches = dynamic_result_matches(
            selected_callable.result,
            call.result.as_ref().map(|result| result.home.scalar_type),
        );
        if !result_matches
            || selected_offset != call.selected_table_byte_offset
            || machine_functions
                .get(&call.dynamic_dispatch.dispatch.realization)
                .is_none()
            || call
                .dynamic_dispatch
                .application
                .realization_callables
                .iter()
                .any(|callable| machine_functions.get(&callable.machine).is_none())
        {
            return Err(invalid());
        }
        validate_instance(
            target,
            function,
            call,
            &call.initial_instance,
            &call.dynamic_dispatch.initial,
        )?;
        validate_instance(
            target,
            function,
            call,
            &call.rebound_instance,
            &call.dynamic_dispatch.rebound,
        )?;
        validate_table_address(target, function, call)?;
        validate_argument(target, function, call)?;
        validate_indirect_call(target, function, call)?;
        if let Some(result) = &call.result {
            let expected_result =
                expected_unit_scalar_result_bytes(target, result).ok_or_else(invalid)?;
            let result_end = result
                .code_offset
                .checked_add(result.byte_count)
                .ok_or_else(invalid)?;
            if result.byte_count != expected_result.len()
                || function.bytes.get(result.code_offset..result_end)
                    != Some(expected_result.as_slice())
                || result_end != operation_end
            {
                return Err(invalid());
            }
        }
        let stack = function_stack.ok_or_else(invalid)?;
        let native_call_end = call
            .indirect_call_offset
            .checked_add(call.indirect_call_byte_count)
            .ok_or_else(invalid)?;
        let outbound_bytes = if let Some(outbound) = call.unit_stack.outbound {
            validate_stack_adjustment_pair(
                target.architecture,
                function.machine,
                Some(omega_target_operations::CallSiteOwner::Operation(
                    call.psi_operation,
                )),
                &function.bytes,
                outbound,
            )?;
            if outbound.release_offset != native_call_end
                || call
                    .result
                    .as_ref()
                    .map_or(operation_end, |result| result.code_offset)
                    != outbound
                        .release_offset
                        .checked_add(outbound.release_byte_count)
                        .ok_or_else(invalid)?
            {
                return Err(invalid());
            }
            outbound.byte_size
        } else if target.architecture == Architecture::Aarch64
            && call
                .result
                .as_ref()
                .map_or(operation_end, |result| result.code_offset)
                == native_call_end
        {
            0
        } else {
            return Err(invalid());
        };
        let linkage = if target.architecture == Architecture::X86_64 {
            8
        } else {
            0
        };
        let live = stack
            .frame_bytes
            .checked_add(outbound_bytes)
            .and_then(|value| value.checked_add(linkage))
            .ok_or_else(invalid)?;
        if !live.is_multiple_of(stack.stack_alignment) {
            return Err(invalid());
        }
        peak = peak.max(live);
        previous_end = Some(operation_end);
    }
    Ok(peak)
}

pub(super) fn validate_stored_dynamic_calls(
    target: NativeTarget,
    function: &MachineCodeFunction,
    machine_functions: &std::collections::BTreeMap<MachineId, &MachineCodeFunction>,
    function_stack: Option<&ObjectUnitStack>,
) -> Result<u32, ObjectError> {
    let mut previous_end = None;
    let mut operations = std::collections::BTreeSet::new();
    let mut establishment_operations = std::collections::BTreeSet::new();
    let mut peak = function_stack.map_or(0, |stack| stack.frame_bytes);
    for call in &function.stored_dynamic_calls {
        let establishment = &call.establishment;
        let invalid = || ObjectError::InvalidDynamicCallEvidence {
            caller: function.machine,
            operation: call.psi_operation,
        };
        let establishment_end = establishment
            .code_offset
            .checked_add(establishment.byte_count)
            .ok_or_else(invalid)?;
        let operation_end = call
            .code_offset
            .checked_add(call.byte_count)
            .ok_or_else(invalid)?;
        if previous_end.is_some_and(|end| end > establishment.code_offset)
            || establishment_end > call.code_offset
            || operation_end > function.bytes.len()
            || establishment.operation_ordinal >= call.operation_ordinal
            || !function
                .provenance
                .operations
                .contains(&establishment.psi_operation)
            || !function.provenance.operations.contains(&call.psi_operation)
            || function
                .semantic_code_attribution
                .iter()
                .filter(|attribution| {
                    attribution.site == SemanticCodeSite::Operation(establishment.psi_operation)
                        && attribution.operation_ordinal == establishment.operation_ordinal
                        && attribution.code_offset == establishment.code_offset
                        && attribution.byte_count == establishment.byte_count
                })
                .count()
                != 1
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
            || !establishment
                .stored
                .has_complete_custody(function.machine, establishment.psi_operation)
            || !call
                .dynamic_dispatch
                .has_complete_custody(function.machine, call.psi_operation)
            || call.dynamic_dispatch.stored != establishment.stored
            || !operations.insert(call.psi_operation)
            || !establishment_operations.insert(establishment.psi_operation)
            || establishment.instance.source.access == psi_terminal::StructuralAccess::Owned
            || establishment.descriptor_abi.instance_byte_offset != 0
            || establishment.descriptor_abi.table_byte_offset != 8
            || establishment.descriptor_abi.word_byte_size != 8
            || establishment.descriptor_abi.total_byte_size != 16
            || establishment.descriptor_abi.byte_alignment != 8
            || establishment.descriptor_home_byte_offset % 8 != 0
            || call.result.home.defining_operation != call.psi_operation
            || call.call_plan.result.as_ref() != Some(&call.result.source)
            || !function.unit_scalar_homes.contains(&call.result.home)
            || call.call_plan.parameters.as_slice()
                != std::slice::from_ref(&call.argument.destination)
        {
            return Err(invalid());
        }
        let selected_row = establishment
            .stored
            .application
            .rows
            .iter()
            .position(|row| {
                row.declaring_trait_identity
                    == call.dynamic_dispatch.dispatch.declaring_trait_identity
                    && row.public_requirement_identity
                        == call.dynamic_dispatch.dispatch.public_requirement_identity
                    && row.requirement_identity
                        == call.dynamic_dispatch.dispatch.requirement_identity
                    && row.realization_identity
                        == call.dynamic_dispatch.dispatch.realization_identity
                    && row.realization_callable_identity.as_deref()
                        == Some(
                            call.dynamic_dispatch
                                .dispatch
                                .realization_callable_identity
                                .as_str(),
                        )
            })
            .ok_or_else(invalid)?;
        let selected_offset = u32::try_from(selected_row)
            .ok()
            .and_then(|row| row.checked_mul(8))
            .ok_or_else(invalid)?;
        let selected_callable = establishment
            .stored
            .application
            .realization_callables
            .iter()
            .find(|callable| {
                callable.machine == call.dynamic_dispatch.dispatch.realization
                    && callable.source_callable_identity
                        == call.dynamic_dispatch.dispatch.realization_callable_identity
            })
            .ok_or_else(invalid)?;
        if !dynamic_result_matches(selected_callable.result, Some(call.result.home.scalar_type))
            || selected_offset != call.selected_table_byte_offset
            || machine_functions
                .get(&call.dynamic_dispatch.dispatch.realization)
                .is_none()
            || establishment
                .stored
                .application
                .realization_callables
                .iter()
                .any(|callable| machine_functions.get(&callable.machine).is_none())
        {
            return Err(invalid());
        }
        validate_stored_instance(target, function, call)?;
        validate_stored_table_address(target, function, call)?;
        validate_stored_argument(target, function, call)?;
        validate_stored_indirect_call(target, function, call)?;
        let expected_result =
            expected_unit_scalar_result_bytes(target, &call.result).ok_or_else(invalid)?;
        let result_end = call
            .result
            .code_offset
            .checked_add(call.result.byte_count)
            .ok_or_else(invalid)?;
        if call.result.byte_count != expected_result.len()
            || function.bytes.get(call.result.code_offset..result_end)
                != Some(expected_result.as_slice())
            || result_end != operation_end
        {
            return Err(invalid());
        }
        let stack = function_stack.ok_or_else(invalid)?;
        let native_call_end = call
            .indirect_call_offset
            .checked_add(call.indirect_call_byte_count)
            .ok_or_else(invalid)?;
        let outbound_bytes = if let Some(outbound) = call.unit_stack.outbound {
            validate_stack_adjustment_pair(
                target.architecture,
                function.machine,
                Some(omega_target_operations::CallSiteOwner::Operation(
                    call.psi_operation,
                )),
                &function.bytes,
                outbound,
            )?;
            if outbound.release_offset != native_call_end
                || call.result.code_offset
                    != outbound
                        .release_offset
                        .checked_add(outbound.release_byte_count)
                        .ok_or_else(invalid)?
            {
                return Err(invalid());
            }
            outbound.byte_size
        } else if target.architecture == Architecture::Aarch64
            && call.result.code_offset == native_call_end
        {
            0
        } else {
            return Err(invalid());
        };
        let linkage = if target.architecture == Architecture::X86_64 {
            8
        } else {
            0
        };
        let live = stack
            .frame_bytes
            .checked_add(outbound_bytes)
            .and_then(|value| value.checked_add(linkage))
            .ok_or_else(invalid)?;
        if !live.is_multiple_of(stack.stack_alignment) {
            return Err(invalid());
        }
        peak = peak.max(live);
        previous_end = Some(operation_end);
    }
    Ok(peak)
}

fn validate_stored_instance(
    target: NativeTarget,
    function: &MachineCodeFunction,
    call: &StoredDynamicCallRecord,
) -> Result<(), ObjectError> {
    let establishment = &call.establishment;
    let record = &establishment.instance;
    let selection = &establishment.stored.selection;
    let invalid = || ObjectError::InvalidDynamicCallEvidence {
        caller: function.machine,
        operation: call.psi_operation,
    };
    let home = function
        .unit_parameter_homes
        .iter()
        .find(|home| home.place == record.source.place)
        .ok_or_else(invalid)?;
    if record.selection_ordinal != selection.ordinal
        || record.source.place != selection.source.place
        || record.source.access != selection.source.access
        || record.source.path != selection.source.path
        || record.source_home_byte_offset != home.byte_offset
        || record.source_home_indirect != home.indirect
        || record.source.source != home.source
        || record.source.source.shape != home.shape
        || record
            .source
            .source_byte_offset
            .checked_add(u32::from(record.source.shape.byte_size))
            .is_none_or(|end| end > u32::from(home.shape.byte_size))
    {
        return Err(invalid());
    }
    let expected =
        expected_instance_bytes(target, record, establishment.descriptor_home_byte_offset)
            .ok_or_else(invalid)?;
    let end = record
        .code_offset
        .checked_add(record.byte_count)
        .ok_or_else(invalid)?;
    if record.byte_count != expected.len()
        || function.bytes.get(record.code_offset..end) != Some(expected.as_slice())
    {
        return Err(invalid());
    }
    Ok(())
}

fn validate_stored_table_address(
    target: NativeTarget,
    function: &MachineCodeFunction,
    call: &StoredDynamicCallRecord,
) -> Result<(), ObjectError> {
    let establishment = &call.establishment;
    let invalid = || ObjectError::InvalidDynamicCallEvidence {
        caller: function.machine,
        operation: call.psi_operation,
    };
    let mut expected = Vec::new();
    match (target.architecture, establishment.table_address.encoding) {
        (
            Architecture::X86_64,
            DynamicTableAddressEncoding::X86_64Relative32 { relocation_offset },
        ) => {
            expected.extend_from_slice(&[0x4c, 0x8d, 0x15]);
            if Some(relocation_offset) != establishment.table_address.code_offset.checked_add(3) {
                return Err(invalid());
            }
            expected.extend_from_slice(&0_i32.to_le_bytes());
            expected_x86_stack_store(
                &mut expected,
                10,
                establishment
                    .descriptor_home_byte_offset
                    .checked_add(8)
                    .ok_or_else(invalid)?,
                8,
            )
            .ok_or_else(invalid)?;
        }
        (
            Architecture::Aarch64,
            DynamicTableAddressEncoding::Aarch64PageAddress {
                page_relocation_offset,
                page_offset_relocation_offset,
            },
        ) => {
            if page_relocation_offset != establishment.table_address.code_offset
                || page_offset_relocation_offset != establishment.table_address.code_offset + 4
            {
                return Err(invalid());
            }
            expected.extend_from_slice(&(0x9000_0000 | 10_u32).to_le_bytes());
            expected.extend_from_slice(&(0x9100_0000 | (10_u32 << 5) | 10_u32).to_le_bytes());
            expected.extend_from_slice(
                &expected_aarch64_stack_access(
                    false,
                    10,
                    establishment
                        .descriptor_home_byte_offset
                        .checked_add(8)
                        .ok_or_else(invalid)?,
                    8,
                )
                .ok_or_else(invalid)?
                .to_le_bytes(),
            );
        }
        _ => return Err(invalid()),
    }
    let end = establishment
        .table_address
        .code_offset
        .checked_add(establishment.table_address.byte_count)
        .ok_or_else(invalid)?;
    if establishment.table_address.byte_count != expected.len()
        || function
            .bytes
            .get(establishment.table_address.code_offset..end)
            != Some(expected.as_slice())
    {
        return Err(invalid());
    }
    Ok(())
}

fn validate_stored_argument(
    target: NativeTarget,
    function: &MachineCodeFunction,
    call: &StoredDynamicCallRecord,
) -> Result<(), ObjectError> {
    let source = &call.establishment.instance.source;
    let argument = &call.argument;
    let invalid = || ObjectError::InvalidDynamicCallEvidence {
        caller: function.machine,
        operation: call.psi_operation,
    };
    if argument.place != source.place
        || argument.access != source.access
        || argument.path != source.path
        || argument.root_structural_type != source.root_structural_type
        || argument.structural_type != source.structural_type
        || argument.shape != source.shape
        || argument.source_byte_offset != source.source_byte_offset
        || argument.source_home_byte_offset != call.establishment.descriptor_home_byte_offset
        || argument.source != source.source
        || argument.destination != source.destination
        || argument.bytes.len() != argument.byte_count
    {
        return Err(invalid());
    }
    let expected = expected_argument_bytes(target, argument).ok_or_else(invalid)?;
    let end = argument
        .code_offset
        .checked_add(argument.byte_count)
        .ok_or_else(invalid)?;
    if argument.byte_count != expected.len()
        || argument.bytes != expected
        || function.bytes.get(argument.code_offset..end) != Some(expected.as_slice())
    {
        return Err(invalid());
    }
    Ok(())
}

fn validate_stored_indirect_call(
    target: NativeTarget,
    function: &MachineCodeFunction,
    call: &StoredDynamicCallRecord,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidDynamicCallEvidence {
        caller: function.machine,
        operation: call.psi_operation,
    };
    let table_home = call
        .argument
        .call_stack_bytes
        .checked_add(call.establishment.descriptor_home_byte_offset)
        .and_then(|value| value.checked_add(8))
        .ok_or_else(invalid)?;
    let mut expected = Vec::new();
    match target.architecture {
        Architecture::X86_64 => {
            expected_x86_stack_load(&mut expected, 11, table_home, 8).ok_or_else(invalid)?;
            expected_x86_memory_load(&mut expected, 11, 11, call.selected_table_byte_offset, 8)
                .ok_or_else(invalid)?;
            let expected_call_offset = call
                .argument
                .code_offset
                .checked_add(call.argument.byte_count)
                .and_then(|offset| offset.checked_add(expected.len()))
                .ok_or_else(invalid)?;
            if call.indirect_call_offset != expected_call_offset
                || call.indirect_call_byte_count != 3
            {
                return Err(invalid());
            }
            expected.extend_from_slice(&[0x41, 0xff, 0xd3]);
        }
        Architecture::Aarch64 => {
            expected.extend_from_slice(
                &expected_aarch64_stack_access(true, 9, table_home, 8)
                    .ok_or_else(invalid)?
                    .to_le_bytes(),
            );
            expected.extend_from_slice(
                &expected_aarch64_memory_access(true, 9, 9, call.selected_table_byte_offset, 8)
                    .ok_or_else(invalid)?
                    .to_le_bytes(),
            );
            let expected_call_offset = call
                .argument
                .code_offset
                .checked_add(call.argument.byte_count)
                .and_then(|offset| offset.checked_add(expected.len()))
                .ok_or_else(invalid)?;
            if call.indirect_call_offset != expected_call_offset
                || call.indirect_call_byte_count != 4
            {
                return Err(invalid());
            }
            expected.extend_from_slice(&(0xd63f_0000 | (9_u32 << 5)).to_le_bytes());
        }
    }
    let start = call
        .argument
        .code_offset
        .checked_add(call.argument.byte_count)
        .ok_or_else(invalid)?;
    let end = start.checked_add(expected.len()).ok_or_else(invalid)?;
    if function.bytes.get(start..end) != Some(expected.as_slice()) {
        return Err(invalid());
    }
    Ok(())
}

fn dynamic_result_matches(
    expected: psi_terminal::ClosedConformanceCallableResult,
    actual: Option<psi_core::ScalarType>,
) -> bool {
    match (expected, actual) {
        (psi_terminal::ClosedConformanceCallableResult::Unit, None) => true,
        (
            psi_terminal::ClosedConformanceCallableResult::I32,
            Some(psi_core::ScalarType::Integer(integer)),
        ) => psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32)
            .is_ok_and(|expected| integer == expected),
        (
            psi_terminal::ClosedConformanceCallableResult::Bool,
            Some(psi_core::ScalarType::Boolean),
        ) => true,
        _ => false,
    }
}

#[cfg(test)]
mod result_class_tests {
    use super::dynamic_result_matches;

    #[test]
    fn unit_and_scalar_result_classes_are_not_substitutable() {
        use psi_terminal::ClosedConformanceCallableResult::{Bool, I32, Unit};

        let i32_type = psi_core::ScalarType::Integer(
            psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32).unwrap(),
        );
        assert!(dynamic_result_matches(Unit, None));
        assert!(dynamic_result_matches(I32, Some(i32_type)));
        assert!(dynamic_result_matches(
            Bool,
            Some(psi_core::ScalarType::Boolean)
        ));
        assert!(!dynamic_result_matches(Unit, Some(i32_type)));
        assert!(!dynamic_result_matches(I32, None));
        assert!(!dynamic_result_matches(
            I32,
            Some(psi_core::ScalarType::Boolean)
        ));
    }
}

fn validate_instance(
    target: NativeTarget,
    function: &MachineCodeFunction,
    call: &DynamicCallRecord,
    record: &DynamicInstanceMaterializationRecord,
    selection: &psi_terminal::TerminalDynamicConformanceSelection,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidDynamicCallEvidence {
        caller: function.machine,
        operation: call.psi_operation,
    };
    let home = function
        .unit_parameter_homes
        .iter()
        .find(|home| home.place == record.source.place)
        .ok_or_else(invalid)?;
    if record.selection_ordinal != selection.ordinal
        || record.source.place != selection.source.place
        || record.source.access != selection.source.access
        || record.source.path != selection.source.path
        || record.source_home_byte_offset != home.byte_offset
        || record.source_home_indirect != home.indirect
        || record.source.source != home.source
        || record.source.source.shape != home.shape
        || record
            .source
            .source_byte_offset
            .checked_add(u32::from(record.source.shape.byte_size))
            .is_none_or(|end| end > u32::from(home.shape.byte_size))
    {
        return Err(invalid());
    }
    let expected = expected_instance_bytes(target, record, call.descriptor_home_byte_offset)
        .ok_or_else(invalid)?;
    let end = record
        .code_offset
        .checked_add(record.byte_count)
        .ok_or_else(invalid)?;
    if record.byte_count != expected.len()
        || function.bytes.get(record.code_offset..end) != Some(expected.as_slice())
    {
        return Err(invalid());
    }
    Ok(())
}

fn expected_instance_bytes(
    target: NativeTarget,
    record: &DynamicInstanceMaterializationRecord,
    descriptor_offset: u32,
) -> Option<Vec<u8>> {
    match target.architecture {
        Architecture::X86_64 => {
            let mut bytes = Vec::new();
            if record.source_home_indirect {
                expected_x86_stack_load(&mut bytes, 11, record.source_home_byte_offset, 8)?;
            } else {
                expected_x86_stack_address(&mut bytes, 11, record.source_home_byte_offset);
            }
            expected_x86_stack_store(&mut bytes, 11, descriptor_offset, 8)?;
            Some(bytes)
        }
        Architecture::Aarch64 => {
            let first = if record.source_home_indirect {
                expected_aarch64_stack_access(true, 9, record.source_home_byte_offset, 8)?
            } else {
                (record.source_home_byte_offset <= 0xfff)
                    .then_some(0x9100_03e0 | (record.source_home_byte_offset << 10) | 9_u32)?
            };
            let store = expected_aarch64_stack_access(false, 9, descriptor_offset, 8)?;
            Some(
                [first, store]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect(),
            )
        }
    }
}

fn validate_table_address(
    target: NativeTarget,
    function: &MachineCodeFunction,
    call: &DynamicCallRecord,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidDynamicCallEvidence {
        caller: function.machine,
        operation: call.psi_operation,
    };
    let mut expected = Vec::new();
    match (target.architecture, call.table_address.encoding) {
        (
            Architecture::X86_64,
            DynamicTableAddressEncoding::X86_64Relative32 { relocation_offset },
        ) => {
            expected.extend_from_slice(&[0x4c, 0x8d, 0x15]);
            let expected_relocation = call.table_address.code_offset.checked_add(3);
            if Some(relocation_offset) != expected_relocation {
                return Err(invalid());
            }
            expected.extend_from_slice(&0_i32.to_le_bytes());
            expected_x86_stack_store(
                &mut expected,
                10,
                call.descriptor_home_byte_offset
                    .checked_add(8)
                    .ok_or_else(invalid)?,
                8,
            )
            .ok_or_else(invalid)?;
        }
        (
            Architecture::Aarch64,
            DynamicTableAddressEncoding::Aarch64PageAddress {
                page_relocation_offset,
                page_offset_relocation_offset,
            },
        ) => {
            if page_relocation_offset != call.table_address.code_offset
                || page_offset_relocation_offset != call.table_address.code_offset + 4
            {
                return Err(invalid());
            }
            expected.extend_from_slice(&(0x9000_0000 | 10_u32).to_le_bytes());
            expected.extend_from_slice(&(0x9100_0000 | (10_u32 << 5) | 10_u32).to_le_bytes());
            expected.extend_from_slice(
                &expected_aarch64_stack_access(
                    false,
                    10,
                    call.descriptor_home_byte_offset
                        .checked_add(8)
                        .ok_or_else(invalid)?,
                    8,
                )
                .ok_or_else(invalid)?
                .to_le_bytes(),
            );
        }
        _ => return Err(invalid()),
    }
    let end = call
        .table_address
        .code_offset
        .checked_add(call.table_address.byte_count)
        .ok_or_else(invalid)?;
    if call.table_address.byte_count != expected.len()
        || function.bytes.get(call.table_address.code_offset..end) != Some(expected.as_slice())
    {
        return Err(invalid());
    }
    Ok(())
}

fn validate_argument(
    target: NativeTarget,
    function: &MachineCodeFunction,
    call: &DynamicCallRecord,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidDynamicCallEvidence {
        caller: function.machine,
        operation: call.psi_operation,
    };
    let argument = &call.argument;
    if argument.place != call.rebound_instance.source.place
        || argument.access != call.rebound_instance.source.access
        || argument.path != call.rebound_instance.source.path
        || argument.root_structural_type != call.rebound_instance.source.root_structural_type
        || argument.structural_type != call.rebound_instance.source.structural_type
        || argument.shape != call.rebound_instance.source.shape
        || argument.source_byte_offset != call.rebound_instance.source.source_byte_offset
        || argument.source_home_byte_offset != call.descriptor_home_byte_offset
        || argument.source != call.rebound_instance.source.source
        || argument.destination != call.rebound_instance.source.destination
        || argument.bytes.len() != argument.byte_count
    {
        return Err(invalid());
    }
    let expected = expected_argument_bytes(target, argument).ok_or_else(invalid)?;
    let end = argument
        .code_offset
        .checked_add(argument.byte_count)
        .ok_or_else(invalid)?;
    if argument.byte_count != expected.len()
        || argument.bytes != expected
        || function.bytes.get(argument.code_offset..end) != Some(expected.as_slice())
    {
        return Err(invalid());
    }
    Ok(())
}

fn expected_argument_bytes(
    target: NativeTarget,
    argument: &omega_machine_code::InternalUnitCallArgumentRecord,
) -> Option<Vec<u8>> {
    let descriptor_home = argument
        .call_stack_bytes
        .checked_add(argument.source_home_byte_offset)?;
    if let [ValueLocation::Indirect { pointer, .. }] = argument.destination.locations.as_slice() {
        return match target.architecture {
            Architecture::X86_64 => {
                let (register, stack) = match *pointer {
                    omega_calling_conventions::IndirectPointerLocation::Register(register) => {
                        (x86_terminal_register(register)?, None)
                    }
                    omega_calling_conventions::IndirectPointerLocation::Stack {
                        stack_byte_offset,
                        ..
                    } => (11, Some(stack_byte_offset)),
                };
                let mut bytes = Vec::new();
                expected_x86_stack_load(&mut bytes, register, descriptor_home, 8)?;
                if argument.source_byte_offset != 0 {
                    bytes.extend_from_slice(&[
                        0x48 | ((register >> 3) & 1),
                        0x81,
                        0xc0 | (register & 7),
                    ]);
                    bytes.extend_from_slice(&argument.source_byte_offset.to_le_bytes());
                }
                if let Some(stack_offset) = stack {
                    expected_x86_stack_store(&mut bytes, register, stack_offset, 8)?;
                }
                Some(bytes)
            }
            Architecture::Aarch64 => {
                let (register, stack) = match *pointer {
                    omega_calling_conventions::IndirectPointerLocation::Register(register) => {
                        (aarch64_terminal_register(register)?, None)
                    }
                    omega_calling_conventions::IndirectPointerLocation::Stack {
                        stack_byte_offset,
                        ..
                    } => (9, Some(stack_byte_offset)),
                };
                let mut words = vec![expected_aarch64_stack_access(
                    true,
                    register,
                    descriptor_home,
                    8,
                )?];
                if argument.source_byte_offset > 0xfff {
                    return None;
                }
                if argument.source_byte_offset != 0 {
                    words.push(
                        0x9100_0000
                            | (argument.source_byte_offset << 10)
                            | (u32::from(register) << 5)
                            | u32::from(register),
                    );
                }
                if let Some(stack_offset) = stack {
                    words.push(expected_aarch64_stack_access(
                        false,
                        register,
                        stack_offset,
                        8,
                    )?);
                }
                Some(words.into_iter().flat_map(u32::to_le_bytes).collect())
            }
        };
    }
    let mut fragments = Vec::new();
    for destination in &argument.destination.locations {
        let (register, stack, value_offset, width) = match *destination {
            ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => (Some(register), None, value_byte_offset, byte_size),
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                ..
            } => (None, Some(stack_byte_offset), value_byte_offset, byte_size),
            ValueLocation::Indirect { .. } => return None,
        };
        fragments.push((register, stack, value_offset, width));
    }
    match target.architecture {
        Architecture::X86_64 => {
            let mut bytes = Vec::new();
            expected_x86_stack_load(&mut bytes, 11, descriptor_home, 8)?;
            for (register, stack, value_offset, width) in fragments {
                let destination = match register {
                    Some(register) => x86_terminal_register(register)?,
                    None => 0,
                };
                let source_offset = argument
                    .source_byte_offset
                    .checked_add(u32::from(value_offset))?;
                expected_x86_memory_load(&mut bytes, destination, 11, source_offset, width)?;
                if let Some(stack_offset) = stack {
                    expected_x86_stack_store(&mut bytes, destination, stack_offset, width)?;
                }
            }
            Some(bytes)
        }
        Architecture::Aarch64 => {
            let mut words = vec![expected_aarch64_stack_access(true, 9, descriptor_home, 8)?];
            for (register, stack, value_offset, width) in fragments {
                let destination = match register {
                    Some(register) => aarch64_terminal_register(register)?,
                    None => 10,
                };
                words.push(expected_aarch64_memory_access(
                    true,
                    destination,
                    9,
                    argument
                        .source_byte_offset
                        .checked_add(u32::from(value_offset))?,
                    width,
                )?);
                if let Some(stack_offset) = stack {
                    words.push(expected_aarch64_stack_access(
                        false,
                        destination,
                        stack_offset,
                        width,
                    )?);
                }
            }
            Some(words.into_iter().flat_map(u32::to_le_bytes).collect())
        }
    }
}

fn validate_indirect_call(
    target: NativeTarget,
    function: &MachineCodeFunction,
    call: &DynamicCallRecord,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidDynamicCallEvidence {
        caller: function.machine,
        operation: call.psi_operation,
    };
    let table_home = call
        .argument
        .call_stack_bytes
        .checked_add(call.descriptor_home_byte_offset)
        .and_then(|value| value.checked_add(8))
        .ok_or_else(invalid)?;
    let mut expected = Vec::new();
    match target.architecture {
        Architecture::X86_64 => {
            expected_x86_stack_load(&mut expected, 11, table_home, 8).ok_or_else(invalid)?;
            expected_x86_memory_load(&mut expected, 11, 11, call.selected_table_byte_offset, 8)
                .ok_or_else(invalid)?;
            let expected_call_offset = call
                .argument
                .code_offset
                .checked_add(call.argument.byte_count)
                .and_then(|offset| offset.checked_add(expected.len()))
                .ok_or_else(invalid)?;
            if call.indirect_call_offset != expected_call_offset
                || call.indirect_call_byte_count != 3
            {
                return Err(invalid());
            }
            expected.extend_from_slice(&[0x41, 0xff, 0xd3]);
        }
        Architecture::Aarch64 => {
            expected.extend_from_slice(
                &expected_aarch64_stack_access(true, 9, table_home, 8)
                    .ok_or_else(invalid)?
                    .to_le_bytes(),
            );
            expected.extend_from_slice(
                &expected_aarch64_memory_access(true, 9, 9, call.selected_table_byte_offset, 8)
                    .ok_or_else(invalid)?
                    .to_le_bytes(),
            );
            let expected_call_offset = call
                .argument
                .code_offset
                .checked_add(call.argument.byte_count)
                .and_then(|offset| offset.checked_add(expected.len()))
                .ok_or_else(invalid)?;
            if call.indirect_call_offset != expected_call_offset
                || call.indirect_call_byte_count != 4
            {
                return Err(invalid());
            }
            expected.extend_from_slice(&(0xd63f_0000 | (9_u32 << 5)).to_le_bytes());
        }
    }
    let start = call
        .argument
        .code_offset
        .checked_add(call.argument.byte_count)
        .ok_or_else(invalid)?;
    let end = start.checked_add(expected.len()).ok_or_else(invalid)?;
    if function.bytes.get(start..end) != Some(expected.as_slice()) {
        return Err(invalid());
    }
    Ok(())
}

fn expected_x86_stack_address(bytes: &mut Vec<u8>, register: u8, byte_offset: u32) {
    bytes.push(0x48 | (((register >> 3) & 1) << 2));
    bytes.push(0x8d);
    if byte_offset == 0 {
        bytes.extend_from_slice(&[0x04 | ((register & 7) << 3), 0x24]);
    } else if byte_offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, byte_offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
}

fn expected_x86_stack_load(
    bytes: &mut Vec<u8>,
    register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Option<()> {
    expected_x86_load_prefix(bytes, register, None, byte_size)?;
    expected_x86_rsp_modrm(bytes, register, byte_offset);
    Some(())
}

fn expected_x86_memory_load(
    bytes: &mut Vec<u8>,
    destination: u8,
    base: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Option<()> {
    expected_x86_load_prefix(bytes, destination, Some(base), byte_size)?;
    if byte_offset == 0 && (base & 7) != 5 {
        bytes.push(((destination & 7) << 3) | (base & 7));
    } else if byte_offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[
            0x40 | ((destination & 7) << 3) | (base & 7),
            byte_offset as u8,
        ]);
    } else {
        bytes.push(0x80 | ((destination & 7) << 3) | (base & 7));
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
    Some(())
}

fn expected_x86_load_prefix(
    bytes: &mut Vec<u8>,
    destination: u8,
    base: Option<u8>,
    byte_size: u16,
) -> Option<()> {
    let rex = 0x40 | (((destination >> 3) & 1) << 2) | base.map_or(0, |base| (base >> 3) & 1);
    match byte_size {
        1 => bytes.extend_from_slice(&[rex, 0x0f, 0xb6]),
        2 => bytes.extend_from_slice(&[0x66, rex, 0x0f, 0xb7]),
        4 => bytes.extend_from_slice(&[rex, 0x8b]),
        8 => bytes.extend_from_slice(&[rex | 0x08, 0x8b]),
        _ => return None,
    }
    Some(())
}

fn expected_x86_stack_store(
    bytes: &mut Vec<u8>,
    register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Option<()> {
    match byte_size {
        1 => bytes.push(0x40 | (((register >> 3) & 1) << 2)),
        2 => bytes.extend_from_slice(&[0x66, 0x40 | (((register >> 3) & 1) << 2)]),
        4 => bytes.push(0x40 | (((register >> 3) & 1) << 2)),
        8 => bytes.push(0x48 | (((register >> 3) & 1) << 2)),
        _ => return None,
    }
    bytes.push(if byte_size == 1 { 0x88 } else { 0x89 });
    expected_x86_rsp_modrm(bytes, register, byte_offset);
    Some(())
}

fn expected_x86_rsp_modrm(bytes: &mut Vec<u8>, register: u8, byte_offset: u32) {
    if byte_offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, byte_offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
}

fn expected_aarch64_stack_access(
    load: bool,
    register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Option<u32> {
    expected_aarch64_memory_access(load, register, 31, byte_offset, byte_size)
}

fn expected_aarch64_memory_access(
    load: bool,
    register: u8,
    base: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Option<u32> {
    let scale = u32::from(byte_size);
    let base_instruction = match (load, byte_size) {
        (false, 1) => 0x3900_0000,
        (true, 1) => 0x3940_0000,
        (false, 2) => 0x7900_0000,
        (true, 2) => 0x7940_0000,
        (false, 4) => 0xb900_0000,
        (true, 4) => 0xb940_0000,
        (false, 8) => 0xf900_0000,
        (true, 8) => 0xf940_0000,
        _ => return None,
    };
    (byte_offset.is_multiple_of(scale) && byte_offset / scale <= 0xfff).then_some(
        base_instruction
            | ((byte_offset / scale) << 10)
            | (u32::from(base) << 5)
            | u32::from(register),
    )
}
