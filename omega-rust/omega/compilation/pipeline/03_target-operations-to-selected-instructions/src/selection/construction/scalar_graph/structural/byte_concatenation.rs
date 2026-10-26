//! Copy two live spans into fresh activation storage, then publish their sum.
//! Capacity bounds writable backing; it never supplies a readable extent.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedMemoryAccessRole,
    VirtualRegisterId, memory,
};
use super::{byte_field_store, byte_views, invalid, local_storage, provenance, transport_register};
use crate::SelectedInstructionError;
use semantic_vocabulary::{IntegerSign, IntegerType, PlaceId, ScalarType, ValueId};
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
        crate::selection::byte_concatenation_input::capacity(function, row).ok_or(invalid())?;
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
    let (_, left_length, _, left_type) = builder.resolve(left.length()).ok_or(invalid())?;
    let (_, right_length, _, right_type) = builder.resolve(right.length()).ok_or(invalid())?;
    if left_type != integer || right_type != integer {
        return Err(invalid());
    }
    let left_pointer = operand_pointer(function, builder, row, left)?;
    let right_pointer = operand_pointer(function, builder, row, right)?;
    let slot = builder.transport.push_structural_local_slot(
        row.operation,
        result.place,
        capacity.checked_add(8).ok_or(invalid())?,
        8,
    );
    let owner = local_storage::address(builder, row, slot, 0, capacity + 8, false)?;
    let payload = transport_register(builder, result.place, 8)?;
    builder.emit_address_offset(8, owner, payload, provenance(row))?;
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
    builder.emit_byte_view_address(payload, left_length, right_destination, provenance(row))?;
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
    builder.emit_store(
        0,
        8,
        owner,
        length,
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
            if !source.path.is_empty() {
                return super::descriptor_operands::backing(function, builder, row, source, &[]);
            }
            let pointer = byte_views::backing_pointer(function, builder, row, source.place)?;
            if let Some(view) = builder.transport.view(source.place).copied() {
                let address = transport_register(builder, source.place, 0)?;
                builder.emit_byte_view_address(
                    pointer,
                    view.byte_offset,
                    address,
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
                    .ok_or(invalid())?;
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

/// The operand's measured byte displacement and every backing root its span
/// reaches. The copy and the measured read roster share this derivation; each
/// caller keeps its own row role.
pub(super) fn operand_span(
    function: &LegalizedScalarFunction,
    builder: &Builder<'_>,
    operand: &ByteSequenceOperand,
) -> Result<(u32, Vec<(PlaceId, ValueId)>), SelectedInstructionError> {
    let offset = match operand {
        ByteSequenceOperand::Field { .. } => {
            crate::selection::byte_concatenation_input::field_offset(function, operand)
                .ok_or(invalid())?
        }
        ByteSequenceOperand::Whole { source, .. } => {
            let (geometry, shape) =
                super::descriptor_operands::source_geometry(function, source).ok_or(invalid())?;
            if shape.is_bounded_owned_byte_sequence() {
                geometry.byte_offset.checked_add(8).ok_or(invalid())?
            } else {
                geometry.byte_offset
            }
        }
    };
    let roots = match operand {
        ByteSequenceOperand::Whole { source, length } if source.path.is_empty() => {
            crate::selection::byte_view_homes::view_backing_roots(
                function,
                &builder.transport.views,
                source.place,
                *length,
            )?
        }
        ByteSequenceOperand::Whole { .. } | ByteSequenceOperand::Field { .. } => None,
    };
    Ok((
        offset,
        roots.unwrap_or_else(|| vec![(operand.source(), operand.length())]),
    ))
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
    let (offset, roots) = operand_span(function, builder, operand)?;
    for (root, length) in roots {
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
        builder.constraints.keys.copy_bytes.ok_or(invalid())?,
        &[source_pointer, destination_pointer, count, cursor, byte],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![operand.length()],
            obligations: vec![*capacity_obligation],
            ..Default::default()
        },
    )
}
