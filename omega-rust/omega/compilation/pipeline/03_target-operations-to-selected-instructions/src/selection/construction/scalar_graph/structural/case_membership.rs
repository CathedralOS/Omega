//! Observe a sum tag without disturbing its payload or ownership.
use super::super::IntegerValue;
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    ScalarType, SelectedInstructionKind, SelectedInstructionProvenance, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::transport_register;

pub(in crate::selection) fn observe(
    source: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::StructuralCaseMembership {
        source: place,
        path,
        case,
        case_tag,
        tag_byte_offset,
        indices,
    } = &row.kind
    else {
        return Err(invalid());
    };
    let result = row.result.ok_or_else(|| invalid())?;
    let Some((expected_tag, expected_offset, elements)) =
        crate::selection::aggregate_result_input::membership_layout(source, *place, path, *case)
    else {
        return Err(invalid());
    };
    if result.scalar_type != ScalarType::Boolean
        || (expected_tag, expected_offset) != (*case_tag, *tag_byte_offset)
        || !crate::structural_inputs::structural_reference_input::runtime_indices_match(
            indices, &elements,
        )
    {
        return Err(invalid());
    }
    let place = *place;
    let tag = transport_register(builder, place, *tag_byte_offset)?;
    let provenance = SelectedInstructionProvenance {
        operations: vec![row.operation],
        values: vec![result.value],
        fuel: row.fuel.clone(),
        ..Default::default()
    };
    if let Some(pointer) =
        super::runtime_address::projection_base(source, builder, row, place, *tag_byte_offset)?
    {
        // A runtime element joins the place pointer before the tag load
        // (`runtime_address`), and its footprint names the element's
        // certificate; a static tag reads its exact place extent.
        let pointer =
            super::runtime_address::scale(builder, row, place, *tag_byte_offset, pointer, indices)?;
        super::runtime_address::footprint(
            builder,
            row,
            place,
            *tag_byte_offset,
            4,
            indices,
            super::runtime_address::Written::None,
        )?;
        builder.emit(
            SelectedInstructionKind::Load32 {
                byte_offset: *tag_byte_offset,
            },
            builder.constraints.keys.load32.ok_or_else(|| invalid())?,
            &[pointer, tag],
            provenance,
        )?;
    } else {
        // Projected tags use addressable aggregate storage, including when the
        // entry ABI supplied value fragments. Only a whole sum can use its
        // first fragment directly; a projection must never fall back to it.
        if !path.is_empty() || *tag_byte_offset != 0 || !indices.is_empty() {
            return Err(invalid());
        }
        // Owned ABI fragments contain value bytes. Narrow the tag before
        // comparing so adjacent payload bits cannot affect membership.
        let fragment = builder
            .transport
            .fragments
            .iter()
            .find(|(stored, offset, _)| *stored == place && *offset == 0)
            .map(|(_, _, register)| *register)
            .ok_or_else(|| invalid())?;
        builder.emit(
            SelectedInstructionKind::ZeroExtendU32,
            builder.constraints.keys.copy_i64,
            &[fragment, tag],
            provenance,
        )?;
    }
    let expected = transport_register(builder, place, 0)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(*case_tag)),
        },
        builder.constraints.keys.materialize_i64,
        &[expected],
        Default::default(),
    )?;
    builder.emit(
        SelectedInstructionKind::CompareI64,
        builder.constraints.keys.compare_i64,
        &[tag, expected],
        Default::default(),
    )?;
    let output = builder.register(result.value, result.definition_site, ScalarType::Boolean)?;
    builder.emit(
        SelectedInstructionKind::MaterializeBooleanEqual,
        builder.constraints.keys.materialize_boolean,
        &[output],
        SelectedInstructionProvenance {
            values: vec![result.value],
            ..Default::default()
        },
    )?;
    Ok(output)
}
