//! Select one element store through a mutable element view.
use super::{
    Builder, IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::transport_register;
use semantic_vocabulary::IntegerValue;
use terminal_psi::StructuralTypeShape;

// Store through the view's original backing pointer at the element's leaf;
// the descriptor itself is unchanged.
pub(super) fn write(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
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
        return Err(invalid());
    };
    let (_, index_register, _, index_type) = builder.resolve(index).ok_or_else(|| invalid())?;
    let (_, value_register, _, value_type) = builder.resolve(value).ok_or_else(|| invalid())?;
    if row.result.is_some()
        || index_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
            )
        || !matches!(value_type, ScalarType::Integer(_) | ScalarType::Boolean)
    {
        return Err(invalid());
    }
    let stride = super::element_views::element_stride(function, builder, destination)
        .ok_or_else(|| invalid())?;
    let Some(signature) = function.structural.as_ref() else {
        return Err(invalid());
    };
    let Some(view_type) =
        crate::selection::established_view_input::element_view_type(function, destination)
    else {
        return Err(invalid());
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
        .ok_or_else(|| invalid())?;
    // Preserve the whole element stride while independently selecting the
    // authored scalar leaf inside each element.
    let (byte_offset, byte_count) =
        crate::structural_inputs::structural_reference_input::element_leaf_geometry(
            element,
            path,
            value_type,
            signature.structural_types.as_slice(),
        )
        .ok_or_else(|| invalid())?;
    let pointer = super::element_views::element_backing_pointer(builder, row, destination)?;
    let view = builder
        .transport
        .element_views
        .iter()
        .find(|view| view.place == destination)
        .copied();
    // i * K <= L * K <= R * K <= root bytes, so the element-to-byte scaling of
    // a checked in-bounds index cannot overflow.
    let stride_register = transport_register(builder, destination, 0)?;
    let scaled_index = transport_register(builder, destination, 0)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(stride)),
        },
        builder.constraints.keys.materialize_i64,
        &[stride_register],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, length],
            obligations: vec![obligation],
            ..Default::default()
        },
    )?;
    builder.emit(
        SelectedInstructionKind::WrappingMultiplyI64,
        builder.constraints.keys.multiply_i64,
        &[index_register, stride_register, scaled_index],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, length],
            obligations: vec![obligation],
            ..Default::default()
        },
    )?;
    let physical_index = if let Some(view) = view {
        let offset = transport_register(builder, destination, 0)?;
        // O + i * K <= O + L * K <= R * K, so only this byte-offset sum is
        // claimed exact.
        builder.emit(
            SelectedInstructionKind::ExactAddI64 {
                obligation,
                accepted_fact,
            },
            builder.constraints.keys.add_i64,
            &[view.byte_offset, scaled_index, offset],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![index, length, view.root_length],
                obligations: vec![obligation],
                ..Default::default()
            },
        )?;
        offset
    } else {
        scaled_index
    };
    memory(
        builder,
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
    let address = transport_register(builder, destination, 0)?;
    builder.emit(
        SelectedInstructionKind::ByteViewAddress,
        builder.constraints.keys.add_i64,
        &[pointer, physical_index, address],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, length],
            obligations: vec![obligation],
            ..Default::default()
        },
    )?;
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size: byte_count,
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
