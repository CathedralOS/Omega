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
    // Admission already required writable custody. Storage selection includes
    // an owner borrowed from a caller, not only an owner created in this frame.
    if crate::selection::established_view_input::readable_byte_type(function, destination).is_none()
        || row.result.is_some()
        || index_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
            )
        || value_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| invalid())?,
            )
    {
        return Err(invalid());
    }
    let pointer = backing_pointer(function, builder, row, destination)?;
    let physical_index = backing_index(builder, row, destination, index_register)?;
    let address = transport_register(builder, destination, 0)?;
    builder.emit_byte_view_address(
        pointer,
        physical_index,
        address,
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
    builder.emit_store(
        0,
        1,
        address,
        value_register,
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
    match &row.kind {
        LegalizedScalarInstructionKind::ByteSequenceRead { source, .. } => {
            if crate::selection::established_view_input::readable_byte_type(function, *source)
                .is_none()
            {
                return Err(invalid());
            }
            byte_sequence_read(function, builder, row)
        }
        LegalizedScalarInstructionKind::ByteSequenceLength {
            source,
            length_byte_offset,
            ..
        } => {
            let (_, terminal_psi::StructuralTypeShape::ByteSequence(carrier)) =
                super::descriptor_operands::source_geometry(function, source).ok_or(invalid())?
            else {
                return Err(invalid());
            };
            if let terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. } = carrier {
                let result = row.result.ok_or(invalid())?;
                let output =
                    builder.register(result.value, result.definition_site, result.scalar_type)?;
                builder.emit_materialize_i64(
                    semantic_vocabulary::IntegerValue::Unsigned(u128::from(capacity)),
                    output,
                    provenance(row),
                )?;
                return Ok(output);
            }
            let expected = if matches!(
                carrier,
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
            ) {
                0
            } else {
                8
            };
            if *length_byte_offset != expected {
                return Err(invalid());
            }
            if source.path.is_empty() && builder.transport.view(source.place).is_some() {
                byte_sequence_length(builder, row, source.place, *length_byte_offset)
            } else {
                super::primitive_locals::read(function, builder, row)
            }
        }
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
    let physical_index = backing_index(builder, row, source, index_register)?;
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

/// Reads and writes index the same retained backing, including nested cuts.
/// The view chain proves O + L <= R <= U64::MAX, and the admitted access
/// proves i < L. Thus O + i < R: only the integer offset sum is claimed
/// exact. Pointer arithmetic and memory-effect roots retain their own owners.
fn backing_index(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    index_register: VirtualRegisterId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let Some(view) = builder.transport.view(place).copied() else {
        return Ok(index_register);
    };
    let (index, length, obligation, accepted_fact) = match row.kind {
        LegalizedScalarInstructionKind::ByteSequenceRead {
            index,
            length,
            obligation,
            accepted_fact,
            ..
        }
        | LegalizedScalarInstructionKind::ByteSequenceWrite {
            index,
            length,
            obligation,
            accepted_fact,
            ..
        } => (index, length, obligation, accepted_fact),
        _ => return Err(invalid()),
    };
    let offset = transport_register(builder, place, 0)?;
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
    Ok(offset)
}

pub(super) fn backing_pointer(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    source: PlaceId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if let Some(view) = builder.transport.view(source) {
        return Ok(view.backing_pointer);
    }
    // Use the stored carrier's geometry for every whole backing too. An
    // inline fixed array is already its payload address; loading its first
    // word would reinterpret bytes as a pointer. Descriptor carriers retain
    // their ordinary pointer load through this same projection operation.
    super::descriptor_operands::backing(
        function,
        builder,
        row,
        &terminal_psi::StructuralArgument {
            place: source,
            path: Vec::new(),
            access: terminal_psi::StructuralAccess::SharedBorrow,
        },
        &[],
    )
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
    if let Some(view) = builder.transport.view(source) {
        let input = view.byte_length;
        let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
        builder.emit_copy_i64(
            input,
            output,
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
    builder.emit_load64(length_byte_offset, input, output, provenance)?;
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
