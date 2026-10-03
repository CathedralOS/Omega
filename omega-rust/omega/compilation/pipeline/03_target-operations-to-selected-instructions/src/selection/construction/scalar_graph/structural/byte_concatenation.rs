//! Copy two live spans into fresh activation storage, then publish their sum.
//! Capacity bounds writable backing; it never supplies a readable extent.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedMemoryAccessRole,
    VirtualRegisterId, memory,
};
use super::{byte_field_store, byte_views, invalid, local_storage, provenance, transport_register};
use crate::SelectedInstructionError;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use terminal_psi::control_flow::ByteSequenceOperand;

pub(super) fn establish(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
        result,
        left,
        right,
        capacity_obligation,
        accepted_fact,
    } = &row.kind
    else {
        return Err(invalid());
    };
    let capacity =
        crate::selection::byte_concatenation_input::capacity(function, row).ok_or_else(invalid)?;
    if builder
        .transport
        .pointers
        .iter()
        .any(|(place, _)| *place == result.place)
    {
        return Err(invalid());
    }
    let integer =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?);
    let (_, left_length, _, left_type) = builder.resolve(left.length()).ok_or_else(invalid)?;
    let (_, right_length, _, right_type) = builder.resolve(right.length()).ok_or_else(invalid)?;
    if left_type != integer || right_type != integer {
        return Err(invalid());
    }
    let left_pointer = operand_pointer(function, builder, row, left)?;
    let right_pointer = operand_pointer(function, builder, row, right)?;
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: result.place,
    };
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: capacity.checked_add(8).ok_or_else(invalid)?,
            alignment: 8,
        });
    let owner = local_storage::address(builder, row, slot, 0, capacity + 8, false)?;
    let payload = transport_register(builder, result.place, 8)?;
    builder.emit(
        SelectedInstructionKind::AddressOffset { byte_offset: 8 },
        builder
            .constraints
            .keys
            .address_offset
            .ok_or_else(invalid)?,
        &[owner, payload],
        provenance(row),
    )?;
    copy(
        function,
        builder,
        row,
        left,
        left_pointer,
        payload,
        left_length,
        capacity,
    )?;
    let right_destination = transport_register(builder, result.place, 8)?;
    builder.emit(
        SelectedInstructionKind::ByteViewAddress,
        builder.constraints.keys.add_i64,
        &[payload, left_length, right_destination],
        provenance(row),
    )?;
    copy(
        function,
        builder,
        row,
        right,
        right_pointer,
        right_destination,
        right_length,
        capacity,
    )?;
    // The accepted mathematical sum is at most the reconstructed capacity,
    // itself representable as u32, so this u64 addition cannot overflow.
    let length = transport_register(builder, result.place, 0)?;
    let evidence = SelectedInstructionProvenance {
        operations: vec![row.operation],
        values: vec![left.length(), right.length()],
        obligations: vec![*capacity_obligation],
        ..Default::default()
    };
    builder.emit(
        SelectedInstructionKind::ExactAddI64 {
            obligation: *capacity_obligation,
            accepted_fact: *accepted_fact,
        },
        builder.constraints.keys.add_i64,
        &[left_length, right_length, length],
        evidence.clone(),
    )?;
    memory(
        builder,
        row,
        result.place,
        0,
        8,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 8,
        },
        builder.constraints.keys.store.ok_or_else(invalid)?,
        &[owner, length],
        SelectedInstructionProvenance {
            fuel: row.fuel.clone(),
            ..evidence
        },
    )?;
    builder.transport.pointers.push((result.place, owner));
    Ok(())
}

pub(super) fn operand_pointer(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    operand: &ByteSequenceOperand,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    match operand {
        ByteSequenceOperand::Whole { source, .. } => {
            let pointer = byte_views::backing_pointer(function, builder, row, *source)?;
            if let Some(view) = builder
                .transport
                .views
                .iter()
                .find(|view| view.place == *source)
                .copied()
            {
                let address = transport_register(builder, *source, 0)?;
                builder.emit(
                    SelectedInstructionKind::ByteViewAddress,
                    builder.constraints.keys.add_i64,
                    &[pointer, view.byte_offset, address],
                    provenance(row),
                )?;
                Ok(address)
            } else {
                Ok(pointer)
            }
        }
        ByteSequenceOperand::Field { source, field, .. } => {
            let geometry =
                crate::selection::field_bytes::FieldBytes::resolve(function, source, *field, &[])
                    .ok_or_else(invalid)?;
            super::byte_field_read::payload_pointer(
                function,
                builder,
                row,
                source.place,
                geometry,
                &[],
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn copy(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    operand: &ByteSequenceOperand,
    source_pointer: VirtualRegisterId,
    destination_pointer: VirtualRegisterId,
    count: VirtualRegisterId,
    capacity: u32,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
        result,
        capacity_obligation,
        accepted_fact,
        ..
    } = &row.kind
    else {
        return Err(invalid());
    };
    let cursor = byte_field_store::scratch(builder, 3)?;
    let byte = byte_field_store::scratch(builder, 4)?;
    let offset = match operand {
        ByteSequenceOperand::Field { .. } => {
            crate::selection::byte_concatenation_input::field_offset(function, operand)
                .ok_or_else(invalid)?
        }
        ByteSequenceOperand::Whole { source, .. } => {
            if crate::selection::byte_concatenation_input::owner_layout_type(function, *source)
                .is_some()
            {
                8
            } else {
                0
            }
        }
    };
    let roots = match operand {
        ByteSequenceOperand::Whole { source, length } => {
            crate::selection::byte_view_homes::view_backing_roots(
                function,
                &builder.transport.views,
                *source,
                *length,
            )?
        }
        ByteSequenceOperand::Field { .. } => None,
    };
    for (root, length) in roots.unwrap_or_else(|| vec![(operand.source(), operand.length())]) {
        memory(
            builder,
            row,
            root,
            offset,
            0,
            SelectedMemoryAccessRole::ReadByteSpan {
                length,
                obligation: *capacity_obligation,
                accepted_fact: *accepted_fact,
            },
        )?;
    }
    // Alias footprints conservatively cover the possible destination range.
    // Exact replay checks each live CopyBytes count and its runtime start; this
    // may-write footprint neither initializes padding nor grants read authority.
    memory(
        builder,
        row,
        result.place,
        8,
        capacity,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit(
        SelectedInstructionKind::CopyBytes,
        builder.constraints.keys.copy_bytes.ok_or_else(invalid)?,
        &[source_pointer, destination_pointer, count, cursor, byte],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![operand.length()],
            obligations: vec![*capacity_obligation],
            ..Default::default()
        },
    )
}
