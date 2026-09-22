//! Exact element-width writes through original borrowed pointers at a proven
//! runtime index. The address model stays explicit: the index scales by the
//! declared element width and joins the referent base before the store.
use super::{
    Builder, IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, StructuralAccess, memory,
    transport_register,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use semantic_vocabulary::IntegerValue;

pub(super) fn emit(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::WriteOnlyIndexedPrimitiveStore {
        destination,
        path,
        index,
        value,
        byte_offset,
        byte_size,
        extent,
        obligation,
        accepted_fact,
    } = &row.kind
    else {
        return Err(invalid());
    };
    let signature = source.structural.as_ref().ok_or_else(invalid)?;
    if !signature.entry_claims.is_empty()
        || crate::structural_inputs::structural_reference_input::indexed_primitive_store(
            destination,
            path,
            value.scalar_type,
            &signature.structural_types,
        ) != Some((*byte_offset, *byte_size, *extent))
        || row.result.is_some()
        || !signature
            .parameters
            .iter()
            .any(|parameter| parameter.semantic == *destination)
        || !matches!(
            destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
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
        .find(|(place, _)| *place == destination.place)
        .map(|(_, register)| *register)
        .ok_or_else(invalid)?;
    let (_, index_register, _, index_type) = builder.resolve(index.value).ok_or_else(invalid)?;
    if index_type != index.scalar_type {
        return Err(invalid());
    }
    let (_, value_register, _, value_type) = builder.resolve(value.value).ok_or_else(invalid)?;
    if value_type != value.scalar_type {
        return Err(invalid());
    }
    // The scaled index is transport, not a source value: `index` is proven
    // inside `extent`, so `index * byte_size` lands inside the array span and
    // the wrapping multiply is exact for every reachable operand.
    let stride = transport_register(builder, destination.place, *byte_offset)?;
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
    let scaled = transport_register(builder, destination.place, *byte_offset)?;
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
    let address = transport_register(builder, destination.place, *byte_offset)?;
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
    memory(
        builder,
        row,
        destination.place,
        *byte_offset,
        u32::from(*byte_size),
        SelectedMemoryAccessRole::WriteIndexedPrimitive {
            index: index.value,
            value: value.value,
            extent: *extent,
            obligation: *obligation,
            accepted_fact: *accepted_fact,
        },
    )?;
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset: *byte_offset,
            byte_size: *byte_size,
        },
        builder.constraints.keys.store.ok_or_else(invalid)?,
        &[address, value_register],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index.value, value.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(())
}
