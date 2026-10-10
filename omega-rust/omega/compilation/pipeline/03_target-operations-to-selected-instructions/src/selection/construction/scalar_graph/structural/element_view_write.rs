//! Select one element store through a mutable element view.
use super::{
    Builder, IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionProvenance,
    SelectedMemoryAccessRole, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::scaled_element_index;
use crate::selection::construction::scalar_graph::structural::transport_register;
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
        || crate::structural_inputs::structural_reference_input::scalar_shape(value_type).is_none()
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
    let physical_index = scaled_element_index(
        builder,
        destination,
        stride,
        index_register,
        index,
        length,
        obligation,
        accepted_fact,
        row.operation,
    )?;
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
    builder.emit_store(
        byte_offset,
        u8::try_from(byte_count).map_err(|_| invalid())?,
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
