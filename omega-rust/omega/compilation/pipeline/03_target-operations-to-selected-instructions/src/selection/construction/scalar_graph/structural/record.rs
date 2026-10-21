//! Complete record storage is published only after all field writes.
use super::super::{IntegerValue, ValueId};
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    PlaceId, SelectedInstructionProvenance, SelectedMemoryAccessRole, VirtualRegisterId, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::local_storage;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::transport_register;
pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let fields = crate::selection::record_input::fields(source, row).ok_or_else(|| invalid())?;
    let LegalizedScalarInstructionKind::EstablishRecord {
        result,
        fields: initializers,
        shape,
    } = &row.kind
    else {
        return Err(invalid());
    };
    let pointer = initialize_storage(row, builder, result.place, *shape)?;
    if shape.byte_size == 0 {
        builder.transport.pointers.push((result.place, pointer));
        return Ok(());
    }
    write_fields(
        source,
        row,
        builder,
        result.place,
        result.structural_type,
        pointer,
        initializers,
        &fields,
    )?;
    builder.transport.pointers.push((result.place, pointer));
    Ok(())
}
/// Allocate the unpublished complete value and initialize every byte,
/// including padding that subsequent whole-value transfers may observe.
pub(super) fn initialize_storage(
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    destination: PlaceId,
    shape: calling_conventions::ValueShape,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    // Even an empty record can be borrowed. Reserve an addressable
    // home without adding a byte to its semantic value or ABI shape.
    let slot = builder.transport.push_structural_local_slot(
        row.operation,
        destination,
        shape.byte_size.max(1),
        shape.alignment,
    );
    let pointer = local_storage::address(builder, row, slot, 0, shape.byte_size.max(1), true)?;
    if shape.byte_size == 0 {
        return Ok(pointer);
    }
    // Initialize padding too: a later whole-value copy may transport every byte.
    let zero = transport_register(builder, destination, 0)?;
    builder.emit_materialize_i64(IntegerValue::Unsigned(0), zero, provenance(row))?;
    let mut offset = 0u32;
    while offset < shape.byte_size {
        let width = chunk(shape.byte_size - offset, offset, offset);
        store(
            builder,
            row,
            destination,
            pointer,
            offset,
            width,
            zero,
            Vec::new(),
            Vec::new(),
        )?;
        offset += u32::from(width);
    }
    Ok(pointer)
}
/// Initialize each owned field from its exact scalar or completed child storage.
/// Case construction uses the same writes after establishing its tag.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_fields(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    destination: PlaceId,
    parent: semantic_vocabulary::StructuralTypeId,
    pointer: VirtualRegisterId,
    initializers: &[terminal_psi::RecordFieldInitializer],
    fields: &[(
        u32,
        calling_conventions::ValueShape,
        Option<semantic_vocabulary::ScalarType>,
    )],
) -> Result<(), SelectedInstructionError> {
    if initializers.len() != fields.len() {
        return Err(invalid());
    }
    for (initializer, (offset, field_shape, expected_scalar)) in
        initializers.iter().zip(fields.iter().copied())
    {
        match &initializer.value {
            terminal_psi::RecordFieldValue::Scalar {
                value,
                range_obligation,
            } => {
                let (_, register, _, scalar) = builder.resolve(*value).ok_or_else(|| invalid())?;
                if Some(scalar) != expected_scalar {
                    return Err(invalid());
                }
                let declarations = &source
                    .structural
                    .as_ref()
                    .ok_or_else(invalid)?
                    .structural_types;
                if let Some(fragments) =
                    crate::structural_inputs::structural_reference_input::scalar_field_fragments(
                        parent,
                        &[],
                        initializer.field,
                        declarations,
                    )
                {
                    super::fragmented_scalar::write(
                        builder,
                        row,
                        destination,
                        pointer,
                        offset,
                        fragments,
                        &[],
                        *value,
                    )?;
                    continue;
                }
                let codec =
                    crate::structural_inputs::structural_reference_input::scalar_field_codec(
                        parent,
                        &[],
                        initializer.field,
                        declarations,
                    );
                if let Some(codec) = codec {
                    let canonical =
                        runtime_abi::ScalarStorageCodec::from_scalar(scalar).ok_or_else(invalid)?;
                    if u32::from(codec.byte_size()) != field_shape.byte_size {
                        return Err(invalid());
                    }
                    if codec != canonical || matches!(codec.byte_size(), 3 | 5 | 6 | 7) {
                        let address = super::encoded_scalar::backing_address(
                            builder,
                            row,
                            destination,
                            pointer,
                            offset,
                        )?;
                        let codec = super::encoded_scalar::constant_codec(
                            builder,
                            row,
                            destination,
                            codec,
                        )?;
                        super::encoded_scalar::write(
                            builder,
                            row,
                            destination,
                            address,
                            codec,
                            abstract_operations::AbstractResult {
                                value: *value,
                                scalar_type: scalar,
                            },
                        )?;
                        continue;
                    }
                }
                if codec.is_none()
                    && crate::selection::scalar_call_abi::scalar_shape(scalar) != Some(field_shape)
                {
                    return Err(invalid());
                }
                let width = u8::try_from(field_shape.byte_size).map_err(|_| invalid())?;
                store(
                    builder,
                    row,
                    destination,
                    pointer,
                    offset,
                    width,
                    register,
                    vec![*value],
                    range_obligation.iter().copied().collect(),
                )?;
            }
            terminal_psi::RecordFieldValue::Structural(argument) => {
                write_structural_operand(
                    source,
                    row,
                    builder,
                    destination,
                    pointer,
                    argument,
                    offset,
                    field_shape,
                )?;
            }
        }
    }
    Ok(())
}

// A completed child can begin at a narrower-aligned field offset than its
// total size suggests. Both ends must use encodable scaled displacements;
// copying eight bytes merely because eight remain can select an impossible
// AArch64 store. Keep wide chunks when both offsets permit them, and never
// round the live extent up into a neighboring field.
pub(super) fn chunk(remaining: u32, source_offset: u32, destination_offset: u32) -> u8 {
    [8u8, 4, 2, 1]
        .into_iter()
        .find(|width| {
            let width = u32::from(*width);
            width <= remaining
                && source_offset.is_multiple_of(width)
                && destination_offset.is_multiple_of(width)
        })
        .unwrap_or(1)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn store(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    offset: u32,
    width: u8,
    value: VirtualRegisterId,
    values: Vec<ValueId>,
    obligations: Vec<semantic_vocabulary::ObligationId>,
) -> Result<(), SelectedInstructionError> {
    let (pointer, displacement) =
        super::memory_pointer(builder, row, place, pointer, offset, u32::from(width))?;
    memory(
        builder,
        row,
        place,
        offset,
        u32::from(width),
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit_store(
        displacement,
        width,
        pointer,
        value,
        SelectedInstructionProvenance {
            values,
            obligations,
            ..provenance(row)
        },
    )?;
    Ok(())
}

/// Copy one completed child into a constructor's unpublished destination.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_structural_operand(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    destination: PlaceId,
    pointer: VirtualRegisterId,
    argument: &terminal_psi::StructuralArgument,
    offset: u32,
    field_shape: calling_conventions::ValueShape,
) -> Result<(), SelectedInstructionError> {
    super::leaf_copy::copy_operand(
        source,
        row,
        builder,
        argument,
        &[],
        super::leaf_copy::CopyEnd {
            place: destination,
            pointer,
            byte_offset: offset,
            elements: &[],
        },
        field_shape,
    )
}
