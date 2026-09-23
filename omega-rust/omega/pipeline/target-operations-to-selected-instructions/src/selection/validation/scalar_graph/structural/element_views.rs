//! Independently replay element observations from descriptor and derived homes.
use super::{
    IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, PlaceId, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, VirtualRegisterId, memory,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use crate::selection::validation::scalar_graph::structural::provenance;
use crate::selection::validation::scalar_graph::structural::result;
use semantic_vocabulary::IntegerValue;
use terminal_psi::StructuralTypeShape;

/// The byte stride of the view's element. A derived view home already carries
/// it; a descriptor source reconstructs it from the declared element type,
/// which must be a primitive scalar on this lane.
pub(super) fn element_stride(
    function: &LegalizedScalarFunction,
    replay: &Replay<'_>,
    source: PlaceId,
) -> Option<u32> {
    if let Some(view) = replay
        .transport
        .element_views
        .iter()
        .find(|view| view.place == source)
    {
        return Some(view.element_stride);
    }
    let view_type = crate::selection::established_view_input::element_view_type(function, source)?;
    let signature = function.structural.as_ref()?;
    let declaration = signature
        .structural_types
        .as_slice()
        .iter()
        .find(|declaration| declaration.id == view_type)?;
    let StructuralTypeShape::ElementView { element } = &declaration.shape else {
        return None;
    };
    let element_declaration = signature
        .structural_types
        .as_slice()
        .iter()
        .find(|declaration| declaration.id == *element)?;
    let StructuralTypeShape::PrimitiveScalar(scalar) = element_declaration.shape else {
        return None;
    };
    let shape = crate::structural_inputs::structural_reference_input::scalar_shape(scalar)?;
    crate::structural_inputs::structural_reference_input::align(
        u32::from(shape.byte_size),
        shape.alignment,
    )
}

pub(in crate::selection) fn element_observation(
    function: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let source = match row.kind {
        LegalizedScalarInstructionKind::ElementViewRead { source, .. }
        | LegalizedScalarInstructionKind::ElementViewLength { source, .. } => source,
        _ => return Err(replay.invalid()),
    };
    // An addressable record is not an element descriptor. Reconstruct the
    // semantic view type before interpreting any pointer home as backing and
    // element extent.
    if crate::selection::established_view_input::element_view_type(function, source).is_none() {
        return Err(replay.invalid());
    }
    match row.kind {
        LegalizedScalarInstructionKind::ElementViewRead { .. } => {
            element_view_read(function, replay, row)
        }
        LegalizedScalarInstructionKind::ElementViewLength {
            source,
            length_byte_offset,
        } => element_view_length(replay, row, source, length_byte_offset),
        _ => Err(replay.invalid()),
    }
}

fn element_view_read(
    function: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::ElementViewRead {
        source,
        index,
        length,
        obligation,
        accepted_fact,
    } = row.kind
    else {
        return Err(replay.invalid());
    };
    let definition = row.result.ok_or_else(|| replay.invalid())?;
    let (_, index_register, _, index_type) =
        replay.resolve(index).ok_or_else(|| replay.invalid())?;
    if index_type
        != ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?,
        )
    {
        return Err(replay.invalid());
    }
    let stride = element_stride(function, replay, source).ok_or_else(|| replay.invalid())?;
    let Some(signature) = function.structural.as_ref() else {
        return Err(replay.invalid());
    };
    // The read result is the view's declared element scalar exactly.
    let Some(view_type) =
        crate::selection::established_view_input::element_view_type(function, source)
    else {
        return Err(replay.invalid());
    };
    let element = signature
        .structural_types
        .as_slice()
        .iter()
        .find(|declaration| declaration.id == view_type)
        .and_then(|declaration| match declaration.shape {
            StructuralTypeShape::ElementView { element } => Some(element),
            _ => None,
        })
        .ok_or_else(|| replay.invalid())?;
    let element_scalar = signature
        .structural_types
        .as_slice()
        .iter()
        .find(|declaration| declaration.id == element)
        .and_then(|declaration| match declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
            _ => None,
        })
        .ok_or_else(|| replay.invalid())?;
    if definition.scalar_type != element_scalar {
        return Err(replay.invalid());
    }
    let pointer = element_backing_pointer(replay, row, source)?;
    let view = replay
        .transport
        .element_views
        .iter()
        .find(|view| view.place == source)
        .copied();
    // i * K <= L * K <= R * K <= root bytes, so the element-to-byte scaling of
    // a checked in-bounds index cannot overflow.
    let stride_register = result(replay, source, 0)?;
    let scaled_index = result(replay, source, 0)?;
    replay.check_instruction(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(stride)),
        },
        replay.constraints.keys.materialize_i64,
        &[stride_register],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, length],
            obligations: vec![obligation],
            ..Default::default()
        },
    )?;
    replay.check_instruction(
        SelectedInstructionKind::WrappingMultiplyI64,
        replay.constraints.keys.multiply_i64,
        &[index_register, stride_register, scaled_index],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, length],
            obligations: vec![obligation],
            ..Default::default()
        },
    )?;
    let physical_index = match view {
        Some(view) => {
            let offset = result(replay, source, 0)?;
            // The replayed view chain gives O + L * K <= R * K <= U64::MAX; this
            // exact view's read fact gives i < L. Thus O + i * K < R * K is an
            // exact U64 sum.
            replay.check_instruction(
                SelectedInstructionKind::ExactAddI64 {
                    obligation,
                    accepted_fact,
                },
                replay.constraints.keys.add_i64,
                &[view.byte_offset, scaled_index, offset],
                &SelectedInstructionProvenance {
                    operations: vec![row.operation],
                    values: vec![index, length, view.root_length],
                    obligations: vec![obligation],
                    ..Default::default()
                },
            )?;
            offset
        }
        None => scaled_index,
    };
    let output = replay.result_register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    memory(
        replay,
        row,
        source,
        0,
        stride,
        SelectedMemoryAccessRole::ReadElementView {
            index,
            length,
            obligation,
            accepted_fact,
        },
    )?;
    let address = result(replay, source, 0)?;
    replay.check_instruction(
        SelectedInstructionKind::ByteViewAddress,
        replay.constraints.keys.add_i64,
        &[pointer, physical_index, address],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, length],
            obligations: vec![obligation],
            ..Default::default()
        },
    )?;
    let (kind, constraint) = match stride {
        1 => (
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            replay
                .constraints
                .keys
                .load8
                .ok_or_else(|| replay.invalid())?,
        ),
        2 => (
            SelectedInstructionKind::Load16 { byte_offset: 0 },
            replay
                .constraints
                .keys
                .load16
                .ok_or_else(|| replay.invalid())?,
        ),
        4 => (
            SelectedInstructionKind::Load32 { byte_offset: 0 },
            replay
                .constraints
                .keys
                .load32
                .ok_or_else(|| replay.invalid())?,
        ),
        8 => (
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            replay
                .constraints
                .keys
                .load64
                .ok_or_else(|| replay.invalid())?,
        ),
        _ => return Err(replay.invalid()),
    };
    replay.check_instruction(
        kind,
        constraint,
        &[address, output],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, length, definition.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}

fn element_view_length(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    source: PlaceId,
    length_byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if length_byte_offset != 8 {
        return Err(replay.invalid());
    }
    let definition = row.result.ok_or_else(|| replay.invalid())?;
    if definition.scalar_type
        != ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?,
        )
    {
        return Err(replay.invalid());
    }
    let output = replay.result_register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    if let Some(view) = replay
        .transport
        .element_views
        .iter()
        .find(|view| view.place == source)
        .copied()
    {
        replay.check_instruction(
            SelectedInstructionKind::CopyI64,
            replay.constraints.keys.copy_i64,
            &[view.element_length, output],
            &SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![definition.value, view.root_length],
                ..Default::default()
            },
        )?;
        return Ok(output);
    }
    let descriptor = replay
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == source)
        .map(|(_, register)| *register)
        .ok_or_else(|| replay.invalid())?;
    memory(
        replay,
        row,
        source,
        8,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Load64 { byte_offset: 8 },
        replay
            .constraints
            .keys
            .load64
            .ok_or_else(|| replay.invalid())?,
        &[descriptor, output],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![definition.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}

/// The root's base address, whether carried by a derived view home or behind a
/// freshly established 16-byte descriptor.
pub(super) fn element_backing_pointer(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    source: PlaceId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if let Some(view) = replay
        .transport
        .element_views
        .iter()
        .find(|view| view.place == source)
    {
        return Ok(view.backing_pointer);
    }
    let descriptor = replay
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == source)
        .map(|(_, register)| *register)
        .ok_or_else(|| replay.invalid())?;
    let pointer = result(replay, source, 0)?;
    memory(
        replay,
        row,
        source,
        0,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        replay
            .constraints
            .keys
            .load64
            .ok_or_else(|| replay.invalid())?,
        &[descriptor, pointer],
        &provenance(row),
    )?;
    Ok(pointer)
}
