//! Exact element-width reads through original borrowed pointers at a proven
//! runtime index. The address model matches the indexed store: the index
//! scales by the declared element width and joins the referent base before
//! the load.
use super::{
    Builder, IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, StructuralAccess, VirtualRegisterId,
    memory, transport_register,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use semantic_vocabulary::IntegerValue;

pub(in crate::selection) fn read(
    source: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::IndexedPrimitiveRead {
        source: argument,
        path,
        index,
        byte_offset,
        byte_size,
        extent,
        obligation,
        accepted_fact,
    } = &row.kind
    else {
        return Err(invalid());
    };
    let definition = row.result.ok_or_else(|| invalid())?;
    let signature = source.structural.as_ref().ok_or_else(|| invalid())?;
    let parameter = signature
        .parameters
        .iter()
        .map(|parameter| &parameter.semantic)
        .find(|parameter| parameter.place == argument.place)
        .ok_or_else(|| invalid())?;
    if !signature.entry_claims.is_empty()
        || parameter.access != argument.access
        || !matches!(
            argument.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
        || crate::structural_inputs::structural_reference_input::indexed_array_layout(
            parameter.structural_type,
            path,
            definition.scalar_type,
            &signature.structural_types,
        ) != Some((*byte_offset, *byte_size, *extent))
        || index.scalar_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
            )
    {
        return Err(invalid());
    }
    let pointer = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == argument.place)
        .map(|(_, register)| *register)
        .ok_or_else(|| invalid())?;
    let (_, index_register, _, index_type) =
        builder.resolve(index.value).ok_or_else(|| invalid())?;
    if index_type != index.scalar_type {
        return Err(invalid());
    }
    // `index` is proven inside `extent`, so `index * byte_size` lands inside
    // the array span and the wrapping multiply is exact for every reachable
    // operand.
    let stride = transport_register(builder, argument.place, *byte_offset)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(*byte_size)),
        },
        builder.constraints.keys.materialize_i64,
        &[stride],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            ..Default::default()
        },
    )?;
    let scaled = transport_register(builder, argument.place, *byte_offset)?;
    builder.emit(
        SelectedInstructionKind::WrappingMultiplyI64,
        builder.constraints.keys.multiply_i64,
        &[index_register, stride, scaled],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index.value],
            ..Default::default()
        },
    )?;
    let address = transport_register(builder, argument.place, *byte_offset)?;
    builder.emit(
        SelectedInstructionKind::ByteViewAddress,
        builder.constraints.keys.add_i64,
        &[pointer, scaled, address],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index.value],
            obligations: vec![*obligation],
            ..Default::default()
        },
    )?;
    let byte_offset = *byte_offset;
    let (instruction, constraint) = match byte_size {
        1 => (
            SelectedInstructionKind::Load8 { byte_offset },
            builder.constraints.keys.load8,
        ),
        2 => (
            SelectedInstructionKind::Load16 { byte_offset },
            builder.constraints.keys.load16,
        ),
        4 => (
            SelectedInstructionKind::Load32 { byte_offset },
            builder.constraints.keys.load32,
        ),
        8 => (
            SelectedInstructionKind::Load64 { byte_offset },
            builder.constraints.keys.load64,
        ),
        _ => return Err(invalid()),
    };
    let output = builder.register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    memory(
        builder,
        row,
        argument.place,
        byte_offset,
        u32::from(*byte_size),
        SelectedMemoryAccessRole::ReadIndexedPrimitive {
            index: index.value,
            extent: *extent,
            obligation: *obligation,
            accepted_fact: *accepted_fact,
        },
    )?;
    builder.emit(
        instruction,
        constraint.ok_or_else(|| invalid())?,
        &[address, output],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index.value, definition.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    super::primitive_locals::normalize_signed_load(builder, definition, output)
}
