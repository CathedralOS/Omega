//! Select element observations from descriptor and derived value homes.
use super::{
    Builder, IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, PlaceId, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, VirtualRegisterId, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::scaled_element_index;
use crate::selection::construction::scalar_graph::structural::transport_register;
use terminal_psi::StructuralTypeShape;

/// The byte stride of the view's element. A derived view home already carries
/// it; a descriptor source reconstructs it from the declared element type's
/// layout (size rounded up to alignment), for scalar and record elements
/// alike. A read selects a scalar leaf within that element below.
pub(super) fn element_stride(
    function: &LegalizedScalarFunction,
    builder: &Builder<'_>,
    source: PlaceId,
) -> Option<u32> {
    if let Some(view) = builder.transport.element_view(source) {
        return Some(view.element_stride);
    }
    let view_type = crate::selection::established_view_input::element_view_type(function, source)?;
    let signature = function.structural.as_ref()?;
    let declaration = signature
        .structural_types
        .as_slice()
        .iter()
        .find(|declaration| declaration.id == view_type)?;
    let StructuralTypeShape::ElementView { element, .. } = &declaration.shape else {
        return None;
    };
    let shape = crate::structural_inputs::structural_reference_input::shape(
        *element,
        signature.structural_types.as_slice(),
    )?;
    crate::structural_inputs::structural_reference_input::align(shape.byte_size, shape.alignment)
        .filter(|stride| *stride != 0)
}

pub(in crate::selection) fn element_observation(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    match &row.kind {
        LegalizedScalarInstructionKind::ElementViewRead { source, .. } => {
            if crate::selection::established_view_input::element_view_type(function, *source)
                .is_none()
            {
                return Err(invalid());
            }
            element_view_read(function, builder, row)
        }
        LegalizedScalarInstructionKind::ElementViewLength {
            source,
            length_byte_offset,
            ..
        } => {
            if *length_byte_offset != 8 {
                return Err(invalid());
            }
            // Derived whole descriptors already carry their exact extent in
            // SSA. Projected descriptors are observations of existing storage,
            // crossing stored references without extracting or copying a view.
            if source.path.is_empty() && builder.transport.element_view(source.place).is_some() {
                element_view_length(builder, row, source.place, *length_byte_offset)
            } else {
                super::primitive_locals::read(function, builder, row)
            }
        }
        _ => Err(invalid()),
    }
}

fn element_view_read(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::ElementViewRead {
        source,
        index,
        length,
        obligation,
        accepted_fact,
        ref path,
        ref indices,
    } = row.kind
    else {
        return Err(invalid());
    };
    let definition = row.result.ok_or_else(|| invalid())?;
    let (_, index_register, _, index_type) = builder.resolve(index).ok_or_else(|| invalid())?;
    if index_type
        != ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?)
    {
        return Err(invalid());
    }
    let stride = element_stride(function, builder, source).ok_or_else(|| invalid())?;
    let Some(signature) = function.structural.as_ref() else {
        return Err(invalid());
    };
    // Reconstruct the leaf independently from the declared element. Its width
    // and offset must not replace the stride used to scale the runtime index.
    let Some(view_type) =
        crate::selection::established_view_input::element_view_type(function, source)
    else {
        return Err(invalid());
    };
    let element = signature
        .structural_types
        .as_slice()
        .iter()
        .find(|declaration| declaration.id == view_type)
        .and_then(|declaration| match declaration.shape {
            StructuralTypeShape::ElementView { element, .. } => Some(element),
            _ => None,
        })
        .ok_or_else(|| invalid())?;
    let (byte_offset, byte_count, elements) =
        crate::structural_inputs::structural_reference_input::element_leaf_projection(
            element,
            path,
            definition.scalar_type,
            signature.structural_types.as_slice(),
        )
        .ok_or_else(|| invalid())?;
    if !crate::structural_inputs::structural_reference_input::runtime_indices_match(
        indices, &elements,
    ) {
        return Err(invalid());
    }
    let pointer = element_backing_pointer(builder, row, source)?;
    let physical_index = scaled_element_index(
        builder,
        source,
        stride,
        index_register,
        index,
        length,
        obligation,
        accepted_fact,
        row.operation,
    )?;
    let output = builder.register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    let address = transport_register(builder, source, 0)?;
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
    let address =
        super::runtime_address::scale(builder, row, source, byte_offset, address, indices)?;
    // A leaf deep inside a large element lies past the target's direct
    // displacement; the address step precedes the access records below.
    let (address, displacement) = super::memory_pointer(
        builder,
        row,
        source,
        address,
        byte_offset,
        u32::from(byte_count),
    )?;
    memory(
        builder,
        row,
        source,
        byte_offset,
        u32::from(byte_count),
        SelectedMemoryAccessRole::ReadElementView {
            index,
            length,
            obligation,
            accepted_fact,
        },
    )?;
    // The final access depends on both the live outer window and every inner
    // fixed-array bound. Record those independent footprints on the load.
    if !indices.is_empty() {
        super::runtime_address::footprint(
            builder,
            row,
            source,
            byte_offset,
            u32::from(byte_count),
            indices,
            super::runtime_address::Written::None,
        )?;
    }
    let (kind, constraint) = match byte_count {
        1 => (
            SelectedInstructionKind::Load8 {
                byte_offset: displacement,
            },
            builder.constraints.keys.load8.ok_or_else(|| invalid())?,
        ),
        2 => (
            SelectedInstructionKind::Load16 {
                byte_offset: displacement,
            },
            builder.constraints.keys.load16.ok_or_else(|| invalid())?,
        ),
        4 => (
            SelectedInstructionKind::Load32 {
                byte_offset: displacement,
            },
            builder.constraints.keys.load32.ok_or_else(|| invalid())?,
        ),
        8 => (
            SelectedInstructionKind::Load64 {
                byte_offset: displacement,
            },
            builder.constraints.keys.load64.ok_or_else(|| invalid())?,
        ),
        _ => return Err(invalid()),
    };
    builder.emit(
        kind,
        constraint,
        &[address, output],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, length, definition.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    // A narrow signed element loads zero-extended; the register carrier is
    // sign-extended, so a signed compare of `-5` would otherwise see 2^32 - 5.
    super::primitive_locals::normalize_signed_load(builder, definition, output)
}

fn element_view_length(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    source: PlaceId,
    length_byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if length_byte_offset != 8 {
        return Err(invalid());
    }
    let definition = row.result.ok_or_else(|| invalid())?;
    if definition.scalar_type
        != ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?)
    {
        return Err(invalid());
    }
    let output = builder.register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    let view = builder
        .transport
        .element_view(source)
        .copied()
        .ok_or_else(|| invalid())?;
    builder.emit_copy_i64(
        view.element_length,
        output,
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![definition.value, view.root_length],
            ..Default::default()
        },
    )?;
    Ok(output)
}

/// The root's base address, whether carried by a derived view home or behind a
/// freshly established 16-byte descriptor.
pub(super) fn element_backing_pointer(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    source: PlaceId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if let Some(view) = builder.transport.element_view(source) {
        return Ok(view.backing_pointer);
    }
    let descriptor = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == source)
        .map(|(_, register)| *register)
        .ok_or_else(|| invalid())?;
    let pointer = transport_register(builder, source, 0)?;
    memory(
        builder,
        row,
        source,
        0,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    builder.emit_load64(0, descriptor, pointer, provenance(row))?;
    Ok(pointer)
}
