//! Independently replay one element store through a mutable element view.
use super::{
    IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, memory,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use crate::selection::validation::scalar_graph::structural::result;
use semantic_vocabulary::IntegerValue;
use terminal_psi::StructuralTypeShape;

// Store through the view's original backing pointer at the element's leaf;
// the descriptor itself is unchanged.
pub(super) fn write(
    function: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::ElementViewWrite {
        ref path,
        destination,
        index,
        value,
        length,
        obligation,
        accepted_fact,
    } = row.kind
    else {
        return Err(replay.invalid());
    };
    let (_, index_register, _, index_type) =
        replay.resolve(index).ok_or_else(|| replay.invalid())?;
    let (_, value_register, _, value_type) =
        replay.resolve(value).ok_or_else(|| replay.invalid())?;
    if row.result.is_some()
        || index_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?,
            )
        || !matches!(value_type, ScalarType::Integer(_) | ScalarType::Boolean)
    {
        return Err(replay.invalid());
    }
    let stride = super::element_views::element_stride(function, replay, destination)
        .ok_or_else(|| replay.invalid())?;
    let Some(signature) = function.structural.as_ref() else {
        return Err(replay.invalid());
    };
    let Some(view_type) =
        crate::selection::established_view_input::element_view_type(function, destination)
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
    // Preserve the whole element stride while independently selecting the
    // authored scalar leaf inside each element.
    let (byte_offset, byte_count) =
        crate::structural_inputs::structural_reference_input::element_leaf_geometry(
            element,
            path,
            value_type,
            signature.structural_types.as_slice(),
        )
        .ok_or_else(|| replay.invalid())?;
    let pointer = super::element_views::element_backing_pointer(replay, row, destination)?;
    let view = replay
        .transport
        .element_views
        .iter()
        .find(|view| view.place == destination)
        .copied();
    // i * K <= L * K <= R * K <= root bytes, so the element-to-byte scaling of
    // a checked in-bounds index cannot overflow.
    let stride_register = result(replay, destination, 0)?;
    let scaled_index = result(replay, destination, 0)?;
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
            let offset = result(replay, destination, 0)?;
            // O + i * K <= O + L * K <= R * K, so only this byte-offset sum is
            // claimed exact.
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
    memory(
        replay,
        row,
        destination,
        byte_offset,
        u32::from(byte_count),
        SelectedMemoryAccessRole::WriteElementView {
            index,
            length,
            obligation,
            accepted_fact,
        },
    )?;
    let address = result(replay, destination, 0)?;
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
    replay.check_instruction(
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size: byte_count,
        },
        replay
            .constraints
            .keys
            .store
            .ok_or_else(|| replay.invalid())?,
        &[address, value_register],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, value, length],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )
}
