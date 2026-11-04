//! Retain each borrowed table operand while selecting the hardware operation.
use super::primitive_locals::borrowed_storage_address;
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedMemoryAccessRole, invalid, memory, provenance,
    transport_register,
};
use crate::SelectedInstructionError;

pub(in crate::selection::construction::scalar_graph) fn emit(
    source: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<bool, SelectedInstructionError> {
    let (operation, key, mut operands) = match &row.kind {
        LegalizedScalarInstructionKind::ActivateTaskRegister {
            entries,
            gdt,
            task_state,
            selector,
            scratch,
            indices,
            ..
        } => {
            let key = builder
                .constraints
                .keys
                .task_register_activate
                .ok_or_else(|| {
                    SelectedInstructionError::custody_because(
                        "task-register activation requires an x86-64 target",
                    )
                })?;
            let (table, table_offset) =
                borrowed_storage_address(source, builder, row, gdt, &indices[0])?;
            let (task, task_offset) =
                borrowed_storage_address(source, builder, row, task_state, &indices[1])?;
            let (snapshot, snapshot_offset) =
                borrowed_storage_address(source, builder, row, scratch, &indices[2])?;
            let (_, selector, _, _) = builder.resolve(*selector).ok_or_else(invalid)?;
            let extent = transport_register(builder, gdt.place, table_offset)?;
            let byte_count = u32::from(*entries) * 8;
            builder.emit_materialize_i64(
                semantic_vocabulary::IntegerValue::Unsigned(u128::from(byte_count)),
                extent,
                provenance(row),
            )?;
            memory(
                builder,
                row,
                gdt.place,
                table_offset,
                byte_count,
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            memory(
                builder,
                row,
                gdt.place,
                table_offset,
                byte_count,
                SelectedMemoryAccessRole::WritePlace,
            )?;
            memory(
                builder,
                row,
                task_state.place,
                task_offset,
                104,
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            memory(
                builder,
                row,
                scratch.place,
                snapshot_offset,
                10,
                SelectedMemoryAccessRole::WritePlace,
            )?;
            (
                terminal_psi::RegisterOperation::TaskRegisterActivate,
                key,
                vec![table, task, selector, snapshot, extent],
            )
        }
        LegalizedScalarInstructionKind::ReloadSegmentDescriptors {
            entries,
            kind,
            gdt,
            selector,
            scratch,
            indices,
            ..
        } => {
            let key = match kind {
                terminal_psi::SegmentReload::Code => builder.constraints.keys.code_segment_reload,
                terminal_psi::SegmentReload::Data => builder.constraints.keys.data_segment_reload,
            }
            .ok_or_else(|| {
                SelectedInstructionError::custody_because(
                    "selector reload requires an x86-64 target",
                )
            })?;
            let (table, table_offset) =
                borrowed_storage_address(source, builder, row, gdt, &indices[0])?;
            let (snapshot, snapshot_offset) =
                borrowed_storage_address(source, builder, row, scratch, &indices[1])?;
            let (_, selector, _, _) = builder.resolve(*selector).ok_or_else(invalid)?;
            let extent = transport_register(builder, gdt.place, table_offset)?;
            let byte_count = u32::from(*entries) * 8;
            builder.emit_materialize_i64(
                semantic_vocabulary::IntegerValue::Unsigned(u128::from(byte_count)),
                extent,
                provenance(row),
            )?;
            memory(
                builder,
                row,
                gdt.place,
                table_offset,
                byte_count,
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            memory(
                builder,
                row,
                gdt.place,
                table_offset,
                byte_count,
                SelectedMemoryAccessRole::WritePlace,
            )?;
            memory(
                builder,
                row,
                scratch.place,
                snapshot_offset,
                10,
                SelectedMemoryAccessRole::WritePlace,
            )?;
            (
                terminal_psi::RegisterOperation::SegmentReload(*kind),
                key,
                vec![table, selector, snapshot, extent],
            )
        }
        LegalizedScalarInstructionKind::ReadSegmentSelector { register, .. } => {
            let key = builder
                .constraints
                .keys
                .segment_selector_read
                .ok_or_else(|| {
                    SelectedInstructionError::custody_because(
                        "selector read requires an x86-64 target",
                    )
                })?;
            (
                terminal_psi::RegisterOperation::SegmentRead(*register),
                key,
                Vec::new(),
            )
        }
        LegalizedScalarInstructionKind::ReadTaskRegister { .. } => {
            let key = builder.constraints.keys.task_register_read.ok_or_else(|| {
                SelectedInstructionError::custody_because(
                    "task-register read requires an x86-64 target",
                )
            })?;
            (
                terminal_psi::RegisterOperation::TaskRegisterRead,
                key,
                Vec::new(),
            )
        }
        _ => return Ok(false),
    };
    let result = row.result.ok_or_else(invalid)?;
    let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
    operands.push(output);
    builder.emit(
        SelectedInstructionKind::RegisterAccess { operation },
        key,
        &operands,
        provenance(row),
    )?;
    builder.definitions.push((
        result.value,
        output,
        result.definition_site,
        result.scalar_type,
    ));
    Ok(true)
}
