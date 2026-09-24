//! Independent replay of indexed primitive reads. The row is re-derived from
//! semantic declarations and SSA definitions: the address staging (stride
//! materialization, scaled index, referent-base address) replays in emitted
//! order before the load.
use super::{
    IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, StructuralAccess, VirtualRegisterId,
    memory, result,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use semantic_vocabulary::IntegerValue;

pub(in crate::selection) fn read(
    source: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
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
        return Err(replay.invalid());
    };
    let definition = row.result.ok_or_else(|| replay.invalid())?;
    let signature = source.structural.as_ref().ok_or_else(|| replay.invalid())?;
    let parameter = signature
        .parameters
        .iter()
        .map(|parameter| &parameter.semantic)
        .find(|parameter| parameter.place == argument.place)
        .ok_or_else(|| replay.invalid())?;
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
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?,
            )
    {
        return Err(replay.invalid());
    }
    let pointer = replay
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == argument.place)
        .map(|(_, register)| *register)
        .ok_or_else(|| replay.invalid())?;
    let (_, index_register, _, index_type) = replay
        .resolve(index.value)
        .ok_or_else(|| replay.invalid())?;
    if index_type != index.scalar_type {
        return Err(replay.invalid());
    }
    let stride = result(replay, argument.place, *byte_offset)?;
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
    let scaled = result(replay, argument.place, *byte_offset)?;
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
    let address = result(replay, argument.place, *byte_offset)?;
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
    let byte_offset = *byte_offset;
    let (instruction, constraint) = match byte_size {
        1 => (
            SelectedInstructionKind::Load8 { byte_offset },
            replay.constraints.keys.load8,
        ),
        2 => (
            SelectedInstructionKind::Load16 { byte_offset },
            replay.constraints.keys.load16,
        ),
        4 => (
            SelectedInstructionKind::Load32 { byte_offset },
            replay.constraints.keys.load32,
        ),
        8 => (
            SelectedInstructionKind::Load64 { byte_offset },
            replay.constraints.keys.load64,
        ),
        _ => return Err(replay.invalid()),
    };
    let output = replay.result_register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    memory(
        replay,
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
    replay.check_instruction(
        instruction,
        constraint.ok_or_else(|| replay.invalid())?,
        &[address, output],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index.value, definition.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    super::primitive_locals::normalize_signed_load(replay, definition, output)
}
