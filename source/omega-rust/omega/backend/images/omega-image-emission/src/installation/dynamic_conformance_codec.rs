//! Canonical installation transport for dynamic-conformance table custody.

use omega_machine_code::InternalUnitScalarCallResultRecord;
use psi_core::{MachineId, OperationId, PlaceId, ValueId};
use psi_terminal::ClosedConformanceApplicationCommitment;

use super::{
    InstallationError, InstalledDynamicCall, InstalledDynamicConformanceSlot,
    InstalledDynamicConformanceTable, InstalledDynamicParameterScalarCall,
    InstalledForwardedDynamicDescriptorAdapter, InstalledForwardedDynamicDescriptorCall,
    InstalledForwardedDynamicDescriptorSlot, InstalledForwardedDynamicDescriptorTable, Reader,
    internal_unit_scalar_call_codec::{decode_offset, encode_offset},
    push_u32, push_u64,
    unit_scalar_codec::{
        decode_scalar_type, decode_unit_scalar_home, encode_scalar_type, encode_unit_scalar_home,
    },
    value_placement_codec::{decode_direct_placement, encode_direct_placement},
};

pub(super) fn encode_dynamic_conformance_custody(
    bytes: &mut Vec<u8>,
    tables: &[InstalledDynamicConformanceTable],
    calls: &[InstalledDynamicCall],
    adapters: &[InstalledForwardedDynamicDescriptorAdapter],
    forwarded_tables: &[InstalledForwardedDynamicDescriptorTable],
    forwarded_calls: &[InstalledForwardedDynamicDescriptorCall],
    parameter_calls: &[InstalledDynamicParameterScalarCall],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(tables.len())
            .map_err(|_| InstallationError::TooManyDynamicConformanceTables)?,
    );
    for table in tables {
        bytes.extend_from_slice(&table.application_commitment.as_bytes());
        push_u64(bytes, table.application_report_fingerprint);
        push_u64(
            bytes,
            u64::try_from(table.data_offset)
                .map_err(|_| InstallationError::InvalidDynamicConformanceTable)?,
        );
        push_u64(
            bytes,
            u64::try_from(table.byte_count)
                .map_err(|_| InstallationError::InvalidDynamicConformanceTable)?,
        );
        push_u32(
            bytes,
            u32::try_from(table.slots.len())
                .map_err(|_| InstallationError::TooManyDynamicConformanceSlots)?,
        );
        for slot in &table.slots {
            push_u32(bytes, slot.row_index);
            bytes.extend_from_slice(&[u8::from(slot.target.is_some()), 0, 0, 0]);
            push_u64(bytes, slot.target.map_or(0, MachineId::get));
            push_u64(
                bytes,
                u64::try_from(slot.data_offset)
                    .map_err(|_| InstallationError::InvalidDynamicConformanceTable)?,
            );
        }
    }

    push_u32(
        bytes,
        u32::try_from(calls.len()).map_err(|_| InstallationError::TooManyDynamicScalarCalls)?,
    );
    for call in calls {
        push_u64(bytes, call.machine.get());
        push_u64(bytes, call.operation.get());
        bytes.extend_from_slice(&call.application_commitment.as_bytes());
        push_u64(bytes, call.initial_source.get());
        push_u64(bytes, call.rebound_source.get());
        push_u32(bytes, call.selected_table_byte_offset);
        push_u32(bytes, 0);
        push_u64(bytes, call.realization.get());
        push_u64(
            bytes,
            u64::try_from(call.text_offset)
                .map_err(|_| InstallationError::InvalidDynamicCall(call.machine))?,
        );
        push_u64(
            bytes,
            u64::try_from(call.byte_count)
                .map_err(|_| InstallationError::InvalidDynamicCall(call.machine))?,
        );
    }
    push_u32(
        bytes,
        u32::try_from(adapters.len())
            .map_err(|_| InstallationError::TooManyForwardedDynamicDescriptorAdapters)?,
    );
    for adapter in adapters {
        bytes.extend_from_slice(&adapter.application_commitment.as_bytes());
        push_u32(bytes, adapter.row_index);
        push_u32(bytes, 0);
        push_u64(bytes, adapter.realization.get());
        push_u64(
            bytes,
            u64::try_from(adapter.text_offset)
                .map_err(|_| InstallationError::InvalidForwardedDynamicDescriptorAdapter)?,
        );
        push_u64(
            bytes,
            u64::try_from(adapter.byte_count)
                .map_err(|_| InstallationError::InvalidForwardedDynamicDescriptorAdapter)?,
        );
    }
    push_u32(
        bytes,
        u32::try_from(forwarded_tables.len())
            .map_err(|_| InstallationError::TooManyForwardedDynamicDescriptorTables)?,
    );
    for table in forwarded_tables {
        bytes.extend_from_slice(&table.application_commitment.as_bytes());
        push_u64(bytes, table.application_report_fingerprint);
        push_u64(
            bytes,
            u64::try_from(table.data_offset)
                .map_err(|_| InstallationError::InvalidForwardedDynamicDescriptorTable)?,
        );
        push_u64(
            bytes,
            u64::try_from(table.byte_count)
                .map_err(|_| InstallationError::InvalidForwardedDynamicDescriptorTable)?,
        );
        push_u32(
            bytes,
            u32::try_from(table.slots.len())
                .map_err(|_| InstallationError::TooManyForwardedDynamicDescriptorSlots)?,
        );
        for slot in &table.slots {
            push_u32(bytes, slot.row_index);
            push_u32(bytes, 0);
            push_u64(bytes, slot.realization.get());
            push_u64(
                bytes,
                u64::try_from(slot.adapter_text_offset)
                    .map_err(|_| InstallationError::InvalidForwardedDynamicDescriptorTable)?,
            );
            push_u64(
                bytes,
                u64::try_from(slot.data_offset)
                    .map_err(|_| InstallationError::InvalidForwardedDynamicDescriptorTable)?,
            );
        }
    }
    push_u32(
        bytes,
        u32::try_from(forwarded_calls.len())
            .map_err(|_| InstallationError::TooManyForwardedDynamicDescriptorCalls)?,
    );
    for call in forwarded_calls {
        push_u64(bytes, call.machine.get());
        push_u64(bytes, call.operation.get());
        push_u64(bytes, call.callee.get());
        bytes.extend_from_slice(&call.application_commitment.as_bytes());
        push_u64(bytes, call.source.get());
        push_u64(bytes, call.semantic_result.value.get());
        encode_scalar_type(bytes, call.semantic_result.scalar_type)?;
        encode_unit_scalar_home(bytes, call.result.home)?;
        encode_direct_placement(bytes, &call.result.source)?;
        encode_offset(bytes, call.result.code_offset)?;
        encode_offset(bytes, call.result.byte_count)?;
        push_u64(
            bytes,
            u64::try_from(call.text_offset).map_err(|_| {
                InstallationError::InvalidForwardedDynamicDescriptorCall(call.machine)
            })?,
        );
        push_u64(
            bytes,
            u64::try_from(call.byte_count).map_err(|_| {
                InstallationError::InvalidForwardedDynamicDescriptorCall(call.machine)
            })?,
        );
    }
    push_u32(
        bytes,
        u32::try_from(parameter_calls.len())
            .map_err(|_| InstallationError::TooManyDynamicParameterScalarCalls)?,
    );
    for call in parameter_calls {
        push_u64(bytes, call.machine.get());
        push_u64(bytes, call.operation.get());
        push_u64(bytes, call.source_value.get());
        push_u32(bytes, call.requirement_slot);
        push_u32(bytes, 0);
        push_u64(
            bytes,
            u64::try_from(call.text_offset)
                .map_err(|_| InstallationError::InvalidDynamicParameterScalarCall(call.machine))?,
        );
        push_u64(
            bytes,
            u64::try_from(call.byte_count)
                .map_err(|_| InstallationError::InvalidDynamicParameterScalarCall(call.machine))?,
        );
    }
    Ok(())
}

pub(super) fn decode_dynamic_conformance_custody(
    reader: &mut Reader<'_>,
) -> Result<
    (
        Vec<InstalledDynamicConformanceTable>,
        Vec<InstalledDynamicCall>,
        Vec<InstalledForwardedDynamicDescriptorAdapter>,
        Vec<InstalledForwardedDynamicDescriptorTable>,
        Vec<InstalledForwardedDynamicDescriptorCall>,
        Vec<InstalledDynamicParameterScalarCall>,
    ),
    InstallationError,
> {
    let table_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyDynamicConformanceTables)?;
    if table_count > reader.remaining() / 60 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut tables = Vec::with_capacity(table_count);
    for _ in 0..table_count {
        let application_commitment =
            ClosedConformanceApplicationCommitment::from_digest(reader.array()?);
        let application_report_fingerprint = reader.u64()?;
        let data_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidDynamicConformanceTable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidDynamicConformanceTable)?;
        let slot_count = usize::try_from(reader.u32()?)
            .map_err(|_| InstallationError::TooManyDynamicConformanceSlots)?;
        if application_commitment.is_zero() || application_report_fingerprint == 0 {
            return Err(InstallationError::InvalidDynamicConformanceTable);
        }
        if slot_count > reader.remaining() / 24 {
            return Err(InstallationError::UnexpectedEnd);
        }
        let mut slots = Vec::with_capacity(slot_count);
        for _ in 0..slot_count {
            let row_index = reader.u32()?;
            let target_present = reader.u8()?;
            if reader.array::<3>()? != [0; 3] {
                return Err(InstallationError::NonzeroReservedField);
            }
            let target_raw = reader.u64()?;
            let target = match target_present {
                0 if target_raw == 0 => None,
                0 => return Err(InstallationError::InvalidDynamicConformanceTable),
                1 => Some(
                    MachineId::new(target_raw)
                        .ok_or(InstallationError::InvalidDynamicConformanceTable)?,
                ),
                tag => return Err(InstallationError::InvalidPresenceFlag(tag)),
            };
            let data_offset = usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::InvalidDynamicConformanceTable)?;
            slots.push(InstalledDynamicConformanceSlot {
                row_index,
                target,
                data_offset,
            });
        }
        tables.push(InstalledDynamicConformanceTable {
            application_commitment,
            application_report_fingerprint,
            data_offset,
            byte_count,
            slots,
        });
    }

    let call_count =
        usize::try_from(reader.u32()?).map_err(|_| InstallationError::TooManyDynamicScalarCalls)?;
    if call_count > reader.remaining() / 96 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut calls = Vec::with_capacity(call_count);
    for _ in 0..call_count {
        let machine =
            MachineId::new(reader.u64()?).ok_or(InstallationError::ZeroFunctionIdentity)?;
        let operation = OperationId::new(reader.u64()?)
            .ok_or(InstallationError::InvalidDynamicCall(machine))?;
        let application_commitment =
            ClosedConformanceApplicationCommitment::from_digest(reader.array()?);
        if application_commitment.is_zero() {
            return Err(InstallationError::InvalidDynamicCall(machine));
        }
        let initial_source =
            PlaceId::new(reader.u64()?).ok_or(InstallationError::InvalidDynamicCall(machine))?;
        let rebound_source =
            PlaceId::new(reader.u64()?).ok_or(InstallationError::InvalidDynamicCall(machine))?;
        let selected_table_byte_offset = reader.u32()?;
        if reader.u32()? != 0 {
            return Err(InstallationError::NonzeroReservedField);
        }
        let realization =
            MachineId::new(reader.u64()?).ok_or(InstallationError::InvalidDynamicCall(machine))?;
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidDynamicCall(machine))?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidDynamicCall(machine))?;
        calls.push(InstalledDynamicCall {
            machine,
            operation,
            application_commitment,
            initial_source,
            rebound_source,
            selected_table_byte_offset,
            realization,
            text_offset,
            byte_count,
        });
    }
    let adapter_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyForwardedDynamicDescriptorAdapters)?;
    if adapter_count > reader.remaining() / 64 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut adapters = Vec::with_capacity(adapter_count);
    for _ in 0..adapter_count {
        let application_commitment =
            ClosedConformanceApplicationCommitment::from_digest(reader.array()?);
        let row_index = reader.u32()?;
        if reader.u32()? != 0 {
            return Err(InstallationError::NonzeroReservedField);
        }
        let realization = MachineId::new(reader.u64()?)
            .ok_or(InstallationError::InvalidForwardedDynamicDescriptorAdapter)?;
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidForwardedDynamicDescriptorAdapter)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidForwardedDynamicDescriptorAdapter)?;
        if application_commitment.is_zero() || byte_count == 0 {
            return Err(InstallationError::InvalidForwardedDynamicDescriptorAdapter);
        }
        adapters.push(InstalledForwardedDynamicDescriptorAdapter {
            application_commitment,
            row_index,
            realization,
            text_offset,
            byte_count,
        });
    }
    let forwarded_table_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyForwardedDynamicDescriptorTables)?;
    let mut forwarded_tables = Vec::with_capacity(forwarded_table_count);
    for _ in 0..forwarded_table_count {
        let application_commitment =
            ClosedConformanceApplicationCommitment::from_digest(reader.array()?);
        let application_report_fingerprint = reader.u64()?;
        let data_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidForwardedDynamicDescriptorTable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidForwardedDynamicDescriptorTable)?;
        let slot_count = usize::try_from(reader.u32()?)
            .map_err(|_| InstallationError::TooManyForwardedDynamicDescriptorSlots)?;
        if slot_count > reader.remaining() / 32
            || application_commitment.is_zero()
            || application_report_fingerprint == 0
        {
            return Err(InstallationError::InvalidForwardedDynamicDescriptorTable);
        }
        let mut slots = Vec::with_capacity(slot_count);
        for _ in 0..slot_count {
            let row_index = reader.u32()?;
            if reader.u32()? != 0 {
                return Err(InstallationError::NonzeroReservedField);
            }
            let realization = MachineId::new(reader.u64()?)
                .ok_or(InstallationError::InvalidForwardedDynamicDescriptorTable)?;
            let adapter_text_offset = usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::InvalidForwardedDynamicDescriptorTable)?;
            let data_offset = usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::InvalidForwardedDynamicDescriptorTable)?;
            slots.push(InstalledForwardedDynamicDescriptorSlot {
                row_index,
                realization,
                adapter_text_offset,
                data_offset,
            });
        }
        forwarded_tables.push(InstalledForwardedDynamicDescriptorTable {
            application_commitment,
            application_report_fingerprint,
            data_offset,
            byte_count,
            slots,
        });
    }
    let forwarded_call_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyForwardedDynamicDescriptorCalls)?;
    if forwarded_call_count > reader.remaining() / 158 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut forwarded_calls = Vec::with_capacity(forwarded_call_count);
    for _ in 0..forwarded_call_count {
        let machine =
            MachineId::new(reader.u64()?).ok_or(InstallationError::ZeroFunctionIdentity)?;
        let operation = OperationId::new(reader.u64()?).ok_or(
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine),
        )?;
        let callee = MachineId::new(reader.u64()?).ok_or(
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine),
        )?;
        let application_commitment =
            ClosedConformanceApplicationCommitment::from_digest(reader.array()?);
        let source = PlaceId::new(reader.u64()?).ok_or(
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine),
        )?;
        let semantic_result = omega_abstract_operations::AbstractResult {
            value: ValueId::new(reader.u64()?).ok_or(
                InstallationError::InvalidForwardedDynamicDescriptorCall(machine),
            )?,
            scalar_type: decode_scalar_type(reader)?,
        };
        let result = InternalUnitScalarCallResultRecord {
            home: decode_unit_scalar_home(reader)?,
            source: decode_direct_placement(reader)?,
            code_offset: decode_offset(reader)?,
            byte_count: decode_offset(reader)?,
        };
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidForwardedDynamicDescriptorCall(machine))?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidForwardedDynamicDescriptorCall(machine))?;
        if application_commitment.is_zero() || byte_count == 0 {
            return Err(InstallationError::InvalidForwardedDynamicDescriptorCall(
                machine,
            ));
        }
        forwarded_calls.push(InstalledForwardedDynamicDescriptorCall {
            machine,
            operation,
            callee,
            application_commitment,
            source,
            semantic_result,
            result,
            text_offset,
            byte_count,
        });
    }
    let parameter_call_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyDynamicParameterScalarCalls)?;
    if parameter_call_count > reader.remaining() / 48 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut parameter_calls = Vec::with_capacity(parameter_call_count);
    for _ in 0..parameter_call_count {
        let machine =
            MachineId::new(reader.u64()?).ok_or(InstallationError::ZeroFunctionIdentity)?;
        let operation = OperationId::new(reader.u64()?).ok_or(
            InstallationError::InvalidDynamicParameterScalarCall(machine),
        )?;
        let source_value = ValueId::new(reader.u64()?).ok_or(
            InstallationError::InvalidDynamicParameterScalarCall(machine),
        )?;
        let requirement_slot = reader.u32()?;
        if reader.u32()? != 0 {
            return Err(InstallationError::NonzeroReservedField);
        }
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidDynamicParameterScalarCall(machine))?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidDynamicParameterScalarCall(machine))?;
        if byte_count == 0 {
            return Err(InstallationError::InvalidDynamicParameterScalarCall(
                machine,
            ));
        }
        parameter_calls.push(InstalledDynamicParameterScalarCall {
            machine,
            operation,
            source_value,
            requirement_slot,
            text_offset,
            byte_count,
        });
    }
    Ok((
        tables,
        calls,
        adapters,
        forwarded_tables,
        forwarded_calls,
        parameter_calls,
    ))
}
