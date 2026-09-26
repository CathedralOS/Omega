//! The `.data` section: dynamic-conformance tables and forwarded descriptor
//! tables, eight zeroed bytes per slot, each slot naming the text symbol the
//! relocation phase binds it to.

use crate::image_emission::object_artifact::replay::dynamic::forwarded_descriptor::ValidatedForwardedDynamicApplication;
use crate::image_emission::object_artifact::{
    ObjectDynamicConformanceSlot, ObjectDynamicConformanceTable, ObjectError,
    ObjectForwardedDynamicDescriptorSlot, ObjectForwardedDynamicDescriptorTable,
};
use crate::object_file::{
    ObjectPlan, ObjectSymbolHandle, SectionKind, SymbolKind, SymbolPlan, SymbolSection,
};
use post_allocation_machine_to_selected_form_encoding::machine_code::ForwardedDynamicDescriptorAdapterIdentity;
use semantic_vocabulary::MachineId;

/// One table per distinct dynamic-conformance application, one slot per row,
/// bound to the realization machine's symbol where the row names one.
pub(super) fn emit_dynamic_conformance_tables(
    dynamic_applications: Vec<terminal_psi::ClosedConformanceApplication>,
    symbols_by_machine: &std::collections::BTreeMap<MachineId, ObjectSymbolHandle>,
    object: &mut ObjectPlan,
    data_bytes: &mut Vec<u8>,
) -> Result<Vec<ObjectDynamicConformanceTable>, ObjectError> {
    let mut dynamic_conformance_tables = Vec::with_capacity(dynamic_applications.len());
    for (table_index, application) in dynamic_applications.into_iter().enumerate() {
        if application.rows.is_empty() {
            return Err(ObjectError::InvalidDynamicConformanceTable);
        }
        let data_offset = data_bytes.len();
        let byte_count = application
            .rows
            .len()
            .checked_mul(8)
            .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?;
        data_bytes.resize(
            data_bytes
                .len()
                .checked_add(byte_count)
                .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
            0,
        );
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: format!(
                "omega_dynamic_conformance_table_{table_index}_{}",
                application.report_fingerprint
            ),
            section: SymbolSection::Section(SectionKind::Data),
            offset: data_offset,
            size: byte_count,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let slots = application
            .rows
            .iter()
            .enumerate()
            .map(|(row_index, row)| {
                let (target, target_symbol) = match &row.realization_callable_identity {
                    Some(callable_identity) => {
                        let matching = application
                            .realization_callables
                            .iter()
                            .filter(|callable| {
                                callable.source_callable_identity == *callable_identity
                            })
                            .collect::<Vec<_>>();
                        let [callable] = matching.as_slice() else {
                            return Err(ObjectError::InvalidDynamicConformanceTable);
                        };
                        let target_symbol =
                            symbols_by_machine.get(&callable.machine).copied().ok_or(
                                ObjectError::UnknownDynamicConformanceTarget(callable.machine),
                            )?;
                        (Some(callable.machine), Some(target_symbol))
                    }
                    None => (None, None),
                };
                Ok(ObjectDynamicConformanceSlot {
                    row_index: u32::try_from(row_index)
                        .map_err(|_| ObjectError::DynamicConformanceDataSizeOverflow)?,
                    realization_callable_identity: row.realization_callable_identity.clone(),
                    target,
                    target_symbol,
                    data_offset: data_offset
                        .checked_add(
                            row_index
                                .checked_mul(8)
                                .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
                        )
                        .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
                })
            })
            .collect::<Result<Vec<_>, ObjectError>>()?;
        dynamic_conformance_tables.push(ObjectDynamicConformanceTable {
            application,
            symbol,
            data_offset,
            byte_count,
            slots,
        });
    }
    Ok(dynamic_conformance_tables)
}

/// One table per forwarded dynamic application, one slot per adapter, bound
/// to that adapter's symbol.
pub(super) fn emit_forwarded_descriptor_tables(
    forwarded_dynamic_applications: &[ValidatedForwardedDynamicApplication],
    forwarded_adapter_symbols: &std::collections::BTreeMap<
        ForwardedDynamicDescriptorAdapterIdentity,
        ObjectSymbolHandle,
    >,
    object: &mut ObjectPlan,
    data_bytes: &mut Vec<u8>,
) -> Result<Vec<ObjectForwardedDynamicDescriptorTable>, ObjectError> {
    let mut forwarded_dynamic_descriptor_tables =
        Vec::with_capacity(forwarded_dynamic_applications.len());
    for application in forwarded_dynamic_applications {
        if application.adapters.is_empty() {
            return Err(ObjectError::InvalidForwardedDynamicDescriptorTable);
        }
        let data_offset = data_bytes.len();
        let byte_count = application
            .adapters
            .len()
            .checked_mul(8)
            .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?;
        data_bytes.resize(
            data_bytes
                .len()
                .checked_add(byte_count)
                .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
            0,
        );
        let commitment = application
            .application
            .commitment
            .as_bytes()
            .into_iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: format!("omega_forwarded_descriptor_table_{commitment}"),
            section: SymbolSection::Section(SectionKind::Data),
            offset: data_offset,
            size: byte_count,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let slots = application
            .adapters
            .iter()
            .enumerate()
            .map(|(row_index, adapter)| {
                let adapter_symbol = forwarded_adapter_symbols
                    .get(&adapter.identity)
                    .copied()
                    .ok_or(ObjectError::InvalidForwardedDynamicDescriptorTable)?;
                Ok(ObjectForwardedDynamicDescriptorSlot {
                    row_index: u32::try_from(row_index)
                        .map_err(|_| ObjectError::DynamicConformanceDataSizeOverflow)?,
                    adapter: adapter.identity.clone(),
                    adapter_symbol,
                    data_offset: data_offset
                        .checked_add(
                            row_index
                                .checked_mul(8)
                                .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
                        )
                        .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
                })
            })
            .collect::<Result<Vec<_>, ObjectError>>()?;
        forwarded_dynamic_descriptor_tables.push(ObjectForwardedDynamicDescriptorTable {
            application: application.application.clone(),
            symbol,
            data_offset,
            byte_count,
            slots,
        });
    }
    Ok(forwarded_dynamic_descriptor_tables)
}
