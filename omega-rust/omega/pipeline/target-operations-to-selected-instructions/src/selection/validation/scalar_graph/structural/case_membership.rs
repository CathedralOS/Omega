//! Independently replay the exact tag read and captured Boolean result.
use super::*;

pub(in crate::selection) fn observe(
    source: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::StructuralCaseMembership {
        source: place,
        case,
        case_tag,
    } = row.kind
    else {
        return Err(replay.invalid());
    };
    let result = row.result.ok_or_else(|| replay.invalid())?;
    if result.scalar_type != ScalarType::Boolean
        || crate::selection::aggregate_result_input::membership_tag(source, place, case)
            != Some(case_tag)
    {
        return Err(replay.invalid());
    }
    let tag = super::result(replay, place, 0)?;
    let provenance = SelectedInstructionProvenance {
        operations: vec![row.operation],
        values: vec![result.value],
        fuel: row.fuel.clone(),
        ..Default::default()
    };
    if let Some(pointer) = replay
        .transport
        .pointers
        .iter()
        .find(|(stored, _)| *stored == place)
        .map(|(_, pointer)| *pointer)
    {
        memory(
            replay,
            row,
            place,
            0,
            4,
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        replay.check_instruction(
            SelectedInstructionKind::Load32 { byte_offset: 0 },
            replay
                .constraints
                .keys
                .load32
                .ok_or_else(|| replay.invalid())?,
            &[pointer, tag],
            &provenance,
        )?;
    } else {
        // Owned ABI fragments contain value bytes. Narrow the tag before
        // comparing so adjacent payload bits cannot affect membership.
        let fragment = replay
            .transport
            .fragments
            .iter()
            .find(|(stored, offset, _)| *stored == place && *offset == 0)
            .map(|(_, _, register)| *register)
            .ok_or_else(|| replay.invalid())?;
        replay.check_instruction(
            SelectedInstructionKind::ZeroExtendU32,
            replay.constraints.keys.copy_i64,
            &[fragment, tag],
            &provenance,
        )?;
    }
    let expected = super::result(replay, place, 0)?;
    replay.check_instruction(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(case_tag)),
        },
        replay.constraints.keys.materialize_i64,
        &[expected],
        &Default::default(),
    )?;
    replay.check_instruction(
        SelectedInstructionKind::CompareI64,
        replay.constraints.keys.compare_i64,
        &[tag, expected],
        &Default::default(),
    )?;
    let output =
        replay.result_register(result.value, result.definition_site, ScalarType::Boolean)?;
    replay.check_instruction(
        SelectedInstructionKind::MaterializeBooleanEqual,
        replay.constraints.keys.materialize_boolean,
        &[output],
        &SelectedInstructionProvenance {
            values: vec![result.value],
            ..Default::default()
        },
    )?;
    Ok(output)
}
