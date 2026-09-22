//! Independent replay of indexed write-only primitive stores. The row is
//! re-derived from semantic declarations and SSA definitions — the address
//! staging (stride materialization, scaled index, referent-base address)
//! replays in emitted order before the store row.
use super::{
    IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, StructuralAccess, memory, result,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use semantic_vocabulary::IntegerValue;

pub(super) fn validate(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    replay: &mut Replay<'_>,
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
        return Err(replay.invalid());
    };
    let signature = source.structural.as_ref().ok_or_else(|| replay.invalid())?;
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
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?,
            )
    {
        return Err(replay.invalid());
    }
    let pointer = replay
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == destination.place)
        .map(|(_, register)| *register)
        .ok_or_else(|| replay.invalid())?;
    let (_, index_register, _, index_type) = replay
        .resolve(index.value)
        .ok_or_else(|| replay.invalid())?;
    if index_type != index.scalar_type {
        return Err(replay.invalid());
    }
    let (_, value_register, _, value_type) = replay
        .resolve(value.value)
        .ok_or_else(|| replay.invalid())?;
    if value_type != value.scalar_type {
        return Err(replay.invalid());
    }
    let stride = result(replay, destination.place, *byte_offset)?;
    replay.check_instruction(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(*byte_size)),
        },
        replay.constraints.keys.materialize_i64,
        &[stride],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            ..Default::default()
        },
    )?;
    let scaled = result(replay, destination.place, *byte_offset)?;
    replay.check_instruction(
        SelectedInstructionKind::WrappingMultiplyI64,
        replay.constraints.keys.multiply_i64,
        &[index_register, stride, scaled],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index.value],
            ..Default::default()
        },
    )?;
    let address = result(replay, destination.place, *byte_offset)?;
    replay.check_instruction(
        SelectedInstructionKind::ByteViewAddress,
        replay.constraints.keys.add_i64,
        &[pointer, scaled, address],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index.value],
            obligations: vec![*obligation],
            ..Default::default()
        },
    )?;
    memory(
        replay,
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
    replay.check_instruction(
        SelectedInstructionKind::Store {
            byte_offset: *byte_offset,
            byte_size: *byte_size,
        },
        replay
            .constraints
            .keys
            .store
            .ok_or_else(|| replay.invalid())?,
        &[address, value_register],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index.value, value.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(())
}
