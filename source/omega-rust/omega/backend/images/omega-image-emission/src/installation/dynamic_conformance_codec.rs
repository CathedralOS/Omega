//! Canonical installation transport for dynamic-conformance table custody.

use psi_core::{MachineId, OperationId, PlaceId};
use psi_terminal::ClosedConformanceApplicationCommitment;

use super::{
    InstallationError, InstalledDynamicConformanceSlot, InstalledDynamicConformanceTable,
    InstalledDynamicScalarCall, Reader, push_u32, push_u64,
};

pub(super) fn encode_dynamic_conformance_custody(
    bytes: &mut Vec<u8>,
    tables: &[InstalledDynamicConformanceTable],
    calls: &[InstalledDynamicScalarCall],
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
                .map_err(|_| InstallationError::InvalidDynamicScalarCall(call.machine))?,
        );
        push_u64(
            bytes,
            u64::try_from(call.byte_count)
                .map_err(|_| InstallationError::InvalidDynamicScalarCall(call.machine))?,
        );
    }
    Ok(())
}

pub(super) fn decode_dynamic_conformance_custody(
    reader: &mut Reader<'_>,
) -> Result<
    (
        Vec<InstalledDynamicConformanceTable>,
        Vec<InstalledDynamicScalarCall>,
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
            .ok_or(InstallationError::InvalidDynamicScalarCall(machine))?;
        let application_commitment =
            ClosedConformanceApplicationCommitment::from_digest(reader.array()?);
        if application_commitment.is_zero() {
            return Err(InstallationError::InvalidDynamicScalarCall(machine));
        }
        let initial_source = PlaceId::new(reader.u64()?)
            .ok_or(InstallationError::InvalidDynamicScalarCall(machine))?;
        let rebound_source = PlaceId::new(reader.u64()?)
            .ok_or(InstallationError::InvalidDynamicScalarCall(machine))?;
        let selected_table_byte_offset = reader.u32()?;
        if reader.u32()? != 0 {
            return Err(InstallationError::NonzeroReservedField);
        }
        let realization = MachineId::new(reader.u64()?)
            .ok_or(InstallationError::InvalidDynamicScalarCall(machine))?;
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidDynamicScalarCall(machine))?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidDynamicScalarCall(machine))?;
        calls.push(InstalledDynamicScalarCall {
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
    Ok((tables, calls))
}
