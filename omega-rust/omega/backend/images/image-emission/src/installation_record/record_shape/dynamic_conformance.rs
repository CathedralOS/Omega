//! The dynamic-conformance and forwarded-descriptor tables: section layout,
//! table and slot ordering, and every dynamic call bound to a table the
//! record actually carries.

use crate::installation_record::{
    InstallationError, InstallationRecord, fingerprint_initialized_data,
    installed_forwarded_dynamic_scalar_result_is_canonical,
};

pub(super) fn validate_installed_dynamic_conformance(
    record: &InstallationRecord,
) -> Result<(), InstallationError> {
    let sections = record.image_sections;
    if sections.text_byte_count == 0
        || sections.layout.text_address == 0
        || sections.final_data_fingerprint.as_bytes() == &[0; 32]
        || (sections.data_byte_count == 0
            && sections.final_data_fingerprint != fingerprint_initialized_data(&[]))
        || sections.final_text_byte_count < sections.text_byte_count
        || sections.final_data_byte_count < sections.data_byte_count
        || sections.executable_inventory_digest.as_bytes() == &[0; 32]
        || sections.data_inventory_digest.as_bytes() == &[0; 32]
    {
        return Err(InstallationError::InvalidImageSectionLayout);
    }
    let functions = record
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut parameter_call_sites = std::collections::BTreeSet::new();
    for call in &record.dynamic_parameter_calls {
        let function = functions
            .get(&call.machine)
            .ok_or(InstallationError::InvalidDynamicParameterCall(call.machine))?;
        let end = call
            .text_offset
            .checked_add(call.byte_count)
            .ok_or(InstallationError::InvalidDynamicParameterCall(call.machine))?;
        let function_end = function
            .text_offset
            .checked_add(function.byte_count)
            .ok_or(InstallationError::InvalidDynamicParameterCall(call.machine))?;
        if call.byte_count == 0
            || call.text_offset < function.text_offset
            || end > function_end
            || !parameter_call_sites.insert((call.machine, call.operation))
        {
            return Err(InstallationError::InvalidDynamicParameterCall(call.machine));
        }
    }
    let forwarded_parameter_machines = record
        .forwarded_dynamic_parameter_calls
        .iter()
        .map(|call| call.machine)
        .collect::<std::collections::BTreeSet<_>>();
    let dynamic_parameter_machines = record
        .dynamic_parameter_calls
        .iter()
        .map(|call| call.machine)
        .collect::<std::collections::BTreeSet<_>>();
    let mut forwarded_parameter_sites = std::collections::BTreeSet::new();
    for call in &record.forwarded_dynamic_parameter_calls {
        let function = functions.get(&call.machine).ok_or(
            InstallationError::InvalidForwardedDynamicParameterCall(call.machine),
        )?;
        let end = call.text_offset.checked_add(call.byte_count).ok_or(
            InstallationError::InvalidForwardedDynamicParameterCall(call.machine),
        )?;
        let function_end = function
            .text_offset
            .checked_add(function.byte_count)
            .ok_or(InstallationError::InvalidForwardedDynamicParameterCall(
                call.machine,
            ))?;
        if call.byte_count == 0
            || call.text_offset < function.text_offset
            || end > function_end
            || call.source_parameter_ordinal != 0
            || call.target_parameter_ordinal != 0
            || !functions.contains_key(&call.callee)
            || (!forwarded_parameter_machines.contains(&call.callee)
                && !dynamic_parameter_machines.contains(&call.callee))
            || !matches!(
                (call.source_value, call.scalar_type),
                (None, None)
                    | (
                        Some(_),
                        Some(
                            semantic_vocabulary::ScalarType::Boolean
                                | semantic_vocabulary::ScalarType::Integer(_)
                        )
                    )
            )
            || !forwarded_parameter_sites.insert((call.machine, call.operation))
        {
            return Err(InstallationError::InvalidForwardedDynamicParameterCall(
                call.machine,
            ));
        }
    }
    if sections.data_byte_count == 0 {
        if !record.dynamic_conformance_tables.is_empty()
            || !record.dynamic_calls.is_empty()
            || !record.stored_dynamic_calls.is_empty()
            || !record.forwarded_dynamic_descriptor_adapters.is_empty()
            || !record.forwarded_dynamic_descriptor_tables.is_empty()
            || !record.forwarded_dynamic_descriptor_calls.is_empty()
        {
            return Err(InstallationError::InvalidImageSectionLayout);
        }
        return Ok(());
    }
    let text_end = sections
        .layout
        .text_address
        .checked_add(
            u64::try_from(sections.text_byte_count)
                .map_err(|_| InstallationError::InvalidImageSectionLayout)?,
        )
        .ok_or(InstallationError::InvalidImageSectionLayout)?;
    if sections.layout.data_address < text_end
        || !sections.layout.data_address.is_multiple_of(8)
        || (record.dynamic_conformance_tables.is_empty()
            && record.forwarded_dynamic_descriptor_tables.is_empty())
    {
        return Err(InstallationError::InvalidImageSectionLayout);
    }

    let mut commitments = std::collections::BTreeSet::new();
    let mut expected_data_offset = 0usize;
    for table in &record.dynamic_conformance_tables {
        let table_byte_count = table
            .slots
            .len()
            .checked_mul(8)
            .ok_or(InstallationError::InvalidDynamicConformanceTable)?;
        if table.application_commitment.is_zero()
            || table.application_report_fingerprint == 0
            || !commitments.insert(table.application_commitment)
            || table.data_offset != expected_data_offset
            || table.byte_count != table_byte_count
            || table.slots.is_empty()
        {
            return Err(InstallationError::InvalidDynamicConformanceTable);
        }
        for (row_index, slot) in table.slots.iter().enumerate() {
            let slot_offset = table
                .data_offset
                .checked_add(
                    row_index
                        .checked_mul(8)
                        .ok_or(InstallationError::InvalidDynamicConformanceTable)?,
                )
                .ok_or(InstallationError::InvalidDynamicConformanceTable)?;
            if usize::try_from(slot.row_index) != Ok(row_index)
                || slot.data_offset != slot_offset
                || slot
                    .target
                    .is_some_and(|target| !functions.contains_key(&target))
            {
                return Err(InstallationError::InvalidDynamicConformanceTable);
            }
        }
        expected_data_offset = expected_data_offset
            .checked_add(table.byte_count)
            .ok_or(InstallationError::InvalidDynamicConformanceTable)?;
    }
    let adapters = record
        .forwarded_dynamic_descriptor_adapters
        .iter()
        .map(|adapter| {
            (
                (
                    adapter.application_commitment,
                    adapter.row_index,
                    adapter.realization,
                ),
                adapter,
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    if adapters.len() != record.forwarded_dynamic_descriptor_adapters.len() {
        return Err(InstallationError::InvalidForwardedDynamicDescriptorAdapter);
    }
    let mut forwarded_commitments = std::collections::BTreeSet::new();
    let mut table_adapter_identities = std::collections::BTreeSet::new();
    for table in &record.forwarded_dynamic_descriptor_tables {
        let table_byte_count = table
            .slots
            .len()
            .checked_mul(8)
            .ok_or(InstallationError::InvalidForwardedDynamicDescriptorTable)?;
        if table.application_commitment.is_zero()
            || table.application_report_fingerprint == 0
            || !forwarded_commitments.insert(table.application_commitment)
            || table.data_offset != expected_data_offset
            || table.byte_count != table_byte_count
            || table.slots.is_empty()
        {
            return Err(InstallationError::InvalidForwardedDynamicDescriptorTable);
        }
        for (row_index, slot) in table.slots.iter().enumerate() {
            let data_offset = table
                .data_offset
                .checked_add(
                    row_index
                        .checked_mul(8)
                        .ok_or(InstallationError::InvalidForwardedDynamicDescriptorTable)?,
                )
                .ok_or(InstallationError::InvalidForwardedDynamicDescriptorTable)?;
            let adapter = adapters
                .get(&(
                    table.application_commitment,
                    slot.row_index,
                    slot.realization,
                ))
                .ok_or(InstallationError::InvalidForwardedDynamicDescriptorTable)?;
            table_adapter_identities.insert((
                table.application_commitment,
                slot.row_index,
                slot.realization,
            ));
            if usize::try_from(slot.row_index) != Ok(row_index)
                || slot.data_offset != data_offset
                || slot.adapter_text_offset != adapter.text_offset
                || !functions.contains_key(&slot.realization)
            {
                return Err(InstallationError::InvalidForwardedDynamicDescriptorTable);
            }
        }
        expected_data_offset = expected_data_offset
            .checked_add(table.byte_count)
            .ok_or(InstallationError::InvalidForwardedDynamicDescriptorTable)?;
    }
    if table_adapter_identities.len() != adapters.len() {
        return Err(InstallationError::InvalidForwardedDynamicDescriptorTable);
    }
    if expected_data_offset != sections.data_byte_count {
        return Err(InstallationError::InvalidDynamicConformanceTable);
    }

    let mut previous_call = None;
    let mut call_sites = std::collections::BTreeSet::new();
    let mut referenced_commitments = std::collections::BTreeSet::new();
    for call in &record.dynamic_calls {
        let function = functions
            .get(&call.machine)
            .ok_or(InstallationError::InvalidDynamicCall(call.machine))?;
        let call_end = call
            .text_offset
            .checked_add(call.byte_count)
            .ok_or(InstallationError::InvalidDynamicCall(call.machine))?;
        let function_end = function
            .text_offset
            .checked_add(function.byte_count)
            .ok_or(InstallationError::InvalidDynamicCall(call.machine))?;
        let table = record
            .dynamic_conformance_tables
            .iter()
            .find(|table| table.application_commitment == call.application_commitment)
            .ok_or(InstallationError::InvalidDynamicCall(call.machine))?;
        referenced_commitments.insert(call.application_commitment);
        let selected_index = usize::try_from(call.selected_table_byte_offset / 8)
            .map_err(|_| InstallationError::InvalidDynamicCall(call.machine))?;
        let selected = table
            .slots
            .get(selected_index)
            .ok_or(InstallationError::InvalidDynamicCall(call.machine))?;
        let order = (call.text_offset, call.machine, call.operation);
        if call.byte_count == 0
            || call.selected_table_byte_offset % 8 != 0
            || call.text_offset < function.text_offset
            || call_end > function_end
            || selected.target != Some(call.realization)
            || !function
                .unit_parameter_homes
                .iter()
                .any(|home| home.place == call.initial_source)
            || !function
                .unit_parameter_homes
                .iter()
                .any(|home| home.place == call.rebound_source)
            || previous_call.is_some_and(|previous| previous >= order)
            || !call_sites.insert((call.machine, call.operation))
        {
            return Err(InstallationError::InvalidDynamicCall(call.machine));
        }
        previous_call = Some(order);
    }
    let mut previous_stored_call = None;
    for call in &record.stored_dynamic_calls {
        let invalid = || InstallationError::InvalidStoredDynamicCall(call.machine);
        let function = functions.get(&call.machine).ok_or_else(invalid)?;
        let establishment_end = call
            .establishment_text_offset
            .checked_add(call.establishment_byte_count)
            .ok_or_else(invalid)?;
        let call_end = call
            .text_offset
            .checked_add(call.byte_count)
            .ok_or_else(invalid)?;
        let function_end = function
            .text_offset
            .checked_add(function.byte_count)
            .ok_or_else(invalid)?;
        let table = record
            .dynamic_conformance_tables
            .iter()
            .find(|table| table.application_commitment == call.application_commitment)
            .ok_or_else(invalid)?;
        referenced_commitments.insert(call.application_commitment);
        let selected_index =
            usize::try_from(call.selected_table_byte_offset / 8).map_err(|_| invalid())?;
        let selected = table.slots.get(selected_index).ok_or_else(invalid)?;
        let order = (
            call.establishment_text_offset,
            call.text_offset,
            call.machine,
            call.operation,
        );
        if call.establishment_byte_count == 0
            || call.byte_count == 0
            || call.selected_table_byte_offset % 8 != 0
            || call.descriptor_home_byte_offset % 8 != 0
            || call.establishment_text_offset < function.text_offset
            || establishment_end > call.text_offset
            || call_end > function_end
            || selected.target != Some(call.realization)
            || !function
                .unit_parameter_homes
                .iter()
                .any(|home| home.place == call.source)
            || previous_stored_call.is_some_and(|previous| previous >= order)
            || !call_sites.insert((call.machine, call.establishment_operation))
            || !call_sites.insert((call.machine, call.operation))
        {
            return Err(invalid());
        }
        previous_stored_call = Some(order);
    }
    if referenced_commitments != commitments {
        return Err(InstallationError::InvalidDynamicConformanceTable);
    }
    let mut forwarded_references = std::collections::BTreeSet::new();
    let mut forwarded_call_sites = std::collections::BTreeSet::new();
    for call in &record.forwarded_dynamic_descriptor_calls {
        let function = functions.get(&call.machine).ok_or(
            InstallationError::InvalidForwardedDynamicDescriptorCall(call.machine),
        )?;
        let end = call.text_offset.checked_add(call.byte_count).ok_or(
            InstallationError::InvalidForwardedDynamicDescriptorCall(call.machine),
        )?;
        let function_end = function
            .text_offset
            .checked_add(function.byte_count)
            .ok_or(InstallationError::InvalidForwardedDynamicDescriptorCall(
                call.machine,
            ))?;
        if call.byte_count == 0
            || call.text_offset < function.text_offset
            || end > function_end
            || !functions.contains_key(&call.callee)
            || !forwarded_commitments.contains(&call.application_commitment)
            || !installed_forwarded_dynamic_scalar_result_is_canonical(
                call,
                function,
                record.target,
            )
            || !forwarded_call_sites.insert((call.machine, call.operation))
        {
            return Err(InstallationError::InvalidForwardedDynamicDescriptorCall(
                call.machine,
            ));
        }
        forwarded_references.insert(call.application_commitment);
    }
    if forwarded_references != forwarded_commitments {
        return Err(InstallationError::InvalidForwardedDynamicDescriptorTable);
    }
    Ok(())
}
