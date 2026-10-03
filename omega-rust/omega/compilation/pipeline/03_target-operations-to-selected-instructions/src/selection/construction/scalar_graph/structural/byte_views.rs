//! Select byte observations from descriptor and derived value homes.
use super::{
    Builder, IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, PlaceId, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, VirtualRegisterId, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::transport_register;
use crate::selection::construction::scalar_graph::structural::{
    byte_concatenation, byte_field_store,
};
use terminal_psi::control_flow::ByteSequenceOperand;

// Store through the original backing pointer; the descriptor itself is unchanged.
pub(super) fn write(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::ByteSequenceWrite {
        destination,
        index,
        value,
        length,
        obligation,
        accepted_fact,
    } = row.kind
    else {
        return Err(invalid());
    };
    let (_, index_register, _, index_type) = builder.resolve(index).ok_or_else(|| invalid())?;
    let (_, value_register, _, value_type) = builder.resolve(value).ok_or_else(|| invalid())?;
    if (crate::selection::established_view_input::view_type(function, destination).is_none()
        && crate::selection::byte_concatenation_input::owned_type(function, destination).is_none())
        || row.result.is_some()
        || index_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
            )
        || value_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| invalid())?,
            )
        || builder
            .transport
            .views
            .iter()
            .any(|view| view.place == destination)
    {
        return Err(invalid());
    }
    let pointer = backing_pointer(function, builder, row, destination)?;
    let address = transport_register(builder, destination, 0)?;
    builder.emit(
        SelectedInstructionKind::ByteViewAddress,
        builder.constraints.keys.add_i64,
        &[pointer, index_register, address],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, length],
            obligations: vec![obligation],
            ..Default::default()
        },
    )?;
    // A block-parameter view destination reaches the roots every incoming
    // edge binds; an un-sliced view place is its own root and keeps the
    // sequence row.
    match crate::selection::byte_view_homes::view_backing_roots(
        function,
        &builder.transport.views,
        destination,
        length,
    )? {
        Some(roots) => {
            for (root, extent) in roots {
                memory(
                    builder,
                    row,
                    root,
                    0,
                    0,
                    SelectedMemoryAccessRole::WriteByteSpan {
                        length: extent,
                        obligation,
                        accepted_fact,
                    },
                )?;
            }
        }
        None => memory(
            builder,
            row,
            destination,
            0,
            1,
            SelectedMemoryAccessRole::WriteByteSequence {
                index,
                value,
                length,
                obligation,
                accepted_fact,
            },
        )?,
    }
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 1,
        },
        builder.constraints.keys.store.ok_or_else(|| invalid())?,
        &[address, value_register],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, value, length],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )
}

pub(in crate::selection) fn byte_observation(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let source = match row.kind {
        LegalizedScalarInstructionKind::ByteSequenceRead { source, .. }
        | LegalizedScalarInstructionKind::ByteSequenceLength { source, .. } => source,
        _ => return Err(invalid()),
    };
    // An addressable record is not a byte descriptor. Reconstruct the semantic
    // view type before interpreting any pointer home as backing and length.
    if crate::selection::established_view_input::readable_byte_type(function, source).is_none() {
        return Err(invalid());
    }
    if let LegalizedScalarInstructionKind::ByteSequenceLength {
        length_byte_offset, ..
    } = row.kind
    {
        let expected =
            if crate::selection::byte_concatenation_input::owner_layout_type(function, source)
                .is_some()
            {
                0
            } else {
                8
            };
        if length_byte_offset != expected {
            return Err(invalid());
        }
    }
    match row.kind {
        LegalizedScalarInstructionKind::ByteSequenceRead { .. } => {
            byte_sequence_read(function, builder, row)
        }
        LegalizedScalarInstructionKind::ByteSequenceLength {
            source,
            length_byte_offset,
        } => byte_sequence_length(builder, row, source, length_byte_offset),
        _ => Err(invalid()),
    }
}

fn byte_sequence_read(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::ByteSequenceRead {
        source,
        index,
        length,
        obligation,
        accepted_fact,
    } = row.kind
    else {
        return Err(invalid());
    };
    let definition = row.result.ok_or_else(|| invalid())?;
    let (_, index_register, _, index_type) = builder.resolve(index).ok_or_else(|| invalid())?;
    if definition.scalar_type
        != ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| invalid())?)
        || index_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
            )
    {
        return Err(invalid());
    }
    let pointer = backing_pointer(function, builder, row, source)?;
    let view = builder
        .transport
        .views
        .iter()
        .find(|view| view.place == source)
        .copied();
    let physical_index = if let Some(view) = view {
        let offset = transport_register(builder, source, 0)?;
        // The view chain proves O + L <= R <= U64::MAX. The checked read i < L
        // gives O + i < R, so only this integer offset sum is claimed exact.
        builder.emit(
            SelectedInstructionKind::ExactAddI64 {
                obligation,
                accepted_fact,
            },
            builder.constraints.keys.add_i64,
            &[view.byte_offset, index_register, offset],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![index, length, view.root_length],
                obligations: vec![obligation],
                ..Default::default()
            },
        )?;
        offset
    } else {
        index_register
    };
    let output = builder.register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    // The payload byte lives in the view's storage root, not the view's own
    // identity. Charge each retained backing root the bound the read reaches
    // so the roster never compares `Place(view)` with `Place(root)` as
    // disjoint storage; an un-sliced view source is its own root and keeps
    // the sequence row.
    match crate::selection::byte_view_homes::view_backing_roots(
        function,
        &builder.transport.views,
        source,
        length,
    )? {
        Some(roots) => {
            for (root, extent) in roots {
                memory(
                    builder,
                    row,
                    root,
                    0,
                    0,
                    SelectedMemoryAccessRole::ReadByteSpan {
                        length: extent,
                        obligation,
                        accepted_fact,
                    },
                )?;
            }
        }
        None => memory(
            builder,
            row,
            source,
            if crate::selection::byte_concatenation_input::owner_layout_type(function, source)
                .is_some()
            {
                8
            } else {
                0
            },
            1,
            SelectedMemoryAccessRole::ReadByteSequence {
                index,
                length,
                obligation,
                accepted_fact,
            },
        )?,
    }
    builder.emit(
        SelectedInstructionKind::Load8Indexed,
        builder
            .constraints
            .keys
            .load8_indexed
            .ok_or_else(|| invalid())?,
        &[pointer, physical_index, output],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, length, definition.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}

pub(super) fn backing_pointer(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    source: PlaceId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if let Some(view) = builder
        .transport
        .views
        .iter()
        .find(|view| view.place == source)
    {
        return Ok(view.backing_pointer);
    }
    let descriptor = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == source)
        .map(|(_, register)| *register)
        .ok_or_else(|| invalid())?;
    if crate::selection::byte_concatenation_input::owner_layout_type(function, source).is_some() {
        let pointer = transport_register(builder, source, 8)?;
        builder.emit(
            SelectedInstructionKind::AddressOffset { byte_offset: 8 },
            builder
                .constraints
                .keys
                .address_offset
                .ok_or_else(|| invalid())?,
            &[descriptor, pointer],
            provenance(row),
        )?;
        return Ok(pointer);
    }
    let pointer = transport_register(builder, source, 0)?;
    memory(
        builder,
        row,
        source,
        0,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        builder.constraints.keys.load64.ok_or_else(|| invalid())?,
        &[descriptor, pointer],
        provenance(row),
    )?;
    Ok(pointer)
}

fn byte_sequence_length(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    source: PlaceId,
    length_byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let result = row.result.ok_or_else(|| invalid())?;
    if !matches!(length_byte_offset, 0 | 8)
        || result.scalar_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
            )
    {
        return Err(invalid());
    }
    if let Some(view) = builder
        .transport
        .views
        .iter()
        .find(|view| view.place == source)
    {
        let input = view.byte_length;
        let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
        builder.emit(
            SelectedInstructionKind::CopyI64,
            builder.constraints.keys.copy_i64,
            &[input, output],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![result.value],
                fuel: row.fuel.clone(),
                ..Default::default()
            },
        )?;
        return Ok(output);
    }
    let input = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == source)
        .map(|(_, register)| *register)
        .ok_or_else(|| invalid())?;
    let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
    memory(
        builder,
        row,
        source,
        length_byte_offset,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    let provenance = SelectedInstructionProvenance {
        operations: vec![row.operation],
        values: vec![result.value],
        fuel: row.fuel.clone(),
        ..Default::default()
    };
    builder.emit(
        SelectedInstructionKind::Load64 {
            byte_offset: length_byte_offset,
        },
        builder.constraints.keys.load64.ok_or_else(|| invalid())?,
        &[input, output],
        provenance,
    )?;
    Ok(output)
}

/// Scan the operand's measured extent and produce the predicate's Boolean.
/// The operand's own dominating length measurement supplies the scan count,
/// so the read is authorized by the operand's construction and the charge
/// names no obligation certificate.
pub(in crate::selection) fn byte_predicate(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::ByteSequencePredicate { operand, predicate } = &row.kind
    else {
        return Err(invalid());
    };
    let result = row.result.ok_or_else(|| invalid())?;
    let integer =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?);
    let (_, count, _, count_type) = builder.resolve(operand.length()).ok_or_else(|| invalid())?;
    if result.scalar_type != ScalarType::Boolean || count_type != integer {
        return Err(invalid());
    }
    let base = byte_concatenation::operand_pointer(function, builder, row, operand)?;
    measured_span_reads(function, builder, row, operand)?;
    let cursor = byte_field_store::scratch(builder, 2)?;
    let byte = byte_field_store::scratch(builder, 3)?;
    let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
    builder.emit(
        SelectedInstructionKind::ScanByteSequence {
            predicate: *predicate,
        },
        builder
            .constraints
            .keys
            .scan_byte_sequence
            .ok_or_else(|| invalid())?,
        &[base, count, cursor, byte, output],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![operand.length(), result.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}

/// Content equality of two measured byte spans. Each operand's own
/// dominating length supplies its read extent; the instruction compares
/// lengths first and scans bytes only when they match.
pub(in crate::selection) fn byte_equal(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::ByteSequenceEqual { left, right } = &row.kind else {
        return Err(invalid());
    };
    let result = row.result.ok_or_else(|| invalid())?;
    let integer =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?);
    let (_, left_count, _, left_count_type) =
        builder.resolve(left.length()).ok_or_else(|| invalid())?;
    let (_, right_count, _, right_count_type) =
        builder.resolve(right.length()).ok_or_else(|| invalid())?;
    if result.scalar_type != ScalarType::Boolean
        || left_count_type != integer
        || right_count_type != integer
    {
        return Err(invalid());
    }
    let left_base = byte_concatenation::operand_pointer(function, builder, row, left)?;
    let right_base = byte_concatenation::operand_pointer(function, builder, row, right)?;
    // Both address computations precede the scan. Attribute both measured
    // reads to the comparison, not the other operand's descriptor load.
    measured_span_reads(function, builder, row, left)?;
    measured_span_reads(function, builder, row, right)?;
    let cursor = byte_field_store::scratch(builder, 4)?;
    let left_byte = byte_field_store::scratch(builder, 5)?;
    let right_byte = byte_field_store::scratch(builder, 6)?;
    let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
    builder.emit(
        SelectedInstructionKind::CompareByteSequences,
        builder
            .constraints
            .keys
            .compare_byte_sequences
            .ok_or_else(|| invalid())?,
        &[
            left_base,
            left_count,
            right_base,
            right_count,
            cursor,
            left_byte,
            right_byte,
            output,
        ],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![left.length(), right.length(), result.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}

/// Record one measured read per backing root at the upcoming scan instruction.
/// Address preparation must be complete before these rows are attributed.
fn measured_span_reads(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    operand: &ByteSequenceOperand,
) -> Result<(), SelectedInstructionError> {
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
            SelectedMemoryAccessRole::ReadMeasuredSpan { length },
        )?;
    }
    Ok(())
}
