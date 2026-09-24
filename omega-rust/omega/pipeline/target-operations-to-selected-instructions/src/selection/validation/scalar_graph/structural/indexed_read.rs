//! Independent replay of indexed primitive reads. The row is re-derived from
//! semantic declarations and SSA definitions — the address staging (stride
//! materialization, scaled index, referent-base address) replays in emitted
//! order before the load row.
use super::{
    IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, StructuralAccess, VirtualRegisterId,
    memory, result,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use semantic_vocabulary::IntegerValue;

pub(in crate::selection) fn validate(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    replay: &mut Replay<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::IndexedPrimitiveRead {
        source: declaration,
        path,
        index,
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
    let result_row = row.result.ok_or_else(|| replay.invalid())?;
    if !signature.entry_claims.is_empty()
        || crate::structural_inputs::structural_reference_input::indexed_primitive_read(
            declaration,
            path,
            result_row.scalar_type,
            &signature.structural_types,
        ) != Some((*byte_offset, *byte_size, *extent))
        || !signature
            .parameters
            .iter()
            .any(|parameter| parameter.semantic == *declaration)
        || !matches!(
            declaration.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
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
        .find(|(place, _)| *place == declaration.place)
        .map(|(_, register)| *register)
        .ok_or_else(|| replay.invalid())?;
    let (_, index_register, _, index_type) = replay
        .resolve(index.value)
        .ok_or_else(|| replay.invalid())?;
    if index_type != index.scalar_type {
        return Err(replay.invalid());
    }
    let output = replay.result_register(
        result_row.value,
        result_row.definition_site,
        result_row.scalar_type,
    )?;
    let stride = result(replay, declaration.place, *byte_offset)?;
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
    let scaled = result(replay, declaration.place, *byte_offset)?;
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
    let address = result(replay, declaration.place, *byte_offset)?;
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
        declaration.place,
        *byte_offset,
        u32::from(*byte_size),
        SelectedMemoryAccessRole::ReadIndexedPrimitive {
            index: index.value,
            extent: *extent,
            obligation: *obligation,
            accepted_fact: *accepted_fact,
        },
    )?;
    let (instruction, constraint) = match byte_size {
        1 => (
            SelectedInstructionKind::Load8 {
                byte_offset: *byte_offset,
            },
            replay.constraints.keys.load8,
        ),
        2 => (
            SelectedInstructionKind::Load16 {
                byte_offset: *byte_offset,
            },
            replay.constraints.keys.load16,
        ),
        4 => (
            SelectedInstructionKind::Load32 {
                byte_offset: *byte_offset,
            },
            replay.constraints.keys.load32,
        ),
        8 => (
            SelectedInstructionKind::Load64 {
                byte_offset: *byte_offset,
            },
            replay.constraints.keys.load64,
        ),
        _ => return Err(replay.invalid()),
    };
    replay.check_instruction(
        instruction,
        constraint.ok_or_else(|| replay.invalid())?,
        &[address, output],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index.value, result_row.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}
