//! Independently replay the tag observation and edge-produced payload roster.
use super::*;
use legalized_operations::{LegalizedScalarBlock, LegalizedScalarTerminator};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedCasePayloadTransport, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedSuccessorRole,
};
use semantic_vocabulary::{IntegerType, PlaceId};

pub(super) fn validate(
    block: &LegalizedScalarBlock,
    replay: &mut Replay<'_>,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let LegalizedScalarTerminator::StructuralCase {
        source: subject,
        layout,
        cases,
        ..
    } = &block.terminator
    else {
        return Err(invalid());
    };
    if cases.len() < 2
        || cases
            .iter()
            .enumerate()
            .any(|(ordinal, case)| usize::try_from(case.case_tag) != Ok(ordinal))
        || layout.tag_byte_offset != 0
        || layout.tag_shape != calling_conventions::ValueShape::integer(4, 4)
    {
        return Err(invalid());
    }
    let slot = match subject {
        legalized_operations::LegalizedStructuralCaseSource::OperationResult {
            operation,
            result,
        } => LocalStorageSlotId::Structural {
            operation: *operation,
            place: result.place,
        },
        legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
            block,
            declaration,
        } => LocalStorageSlotId::StructuralBlockParameter {
            block: *block,
            place: declaration.place,
        },
    };
    if replay
        .transport
        .local_slots
        .iter()
        .filter(|row| {
            row.id == slot
                && row.byte_size == u32::from(layout.shape.byte_size)
                && row.alignment == layout.shape.alignment
        })
        .count()
        != 1
    {
        return Err(invalid());
    }
    let address = temporary(replay, subject.place(), 0, false)?;
    memory(
        replay,
        block.id,
        subject.place(),
        0,
        u32::from(layout.shape.byte_size),
        SelectedMemoryAccessRole::AddressLocal { slot },
    )?;
    replay.check_instruction(
        SelectedInstructionKind::FrameAddress {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset: 0,
        },
        replay.constraints.keys.frame_address.ok_or_else(invalid)?,
        &[address],
        &Default::default(),
    )?;
    let tag = temporary(replay, subject.place(), 0, true)?;
    memory(
        replay,
        block.id,
        subject.place(),
        0,
        4,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Load32 { byte_offset: 0 },
        replay.constraints.keys.load32.ok_or_else(invalid)?,
        &[address, tag],
        &Default::default(),
    )?;
    let original_block = replay.block;
    let mut original_cursor = None;
    for ordinal in 0..cases.len() - 1 {
        if ordinal == 0 {
            replay.check_instruction(
                SelectedInstructionKind::CompareI64Zero,
                replay.constraints.keys.compare_i64_zero,
                &[tag],
                &Default::default(),
            )?;
        } else {
            let expected = temporary(replay, subject.place(), 0, false)?;
            replay.check_instruction(
                SelectedInstructionKind::MaterializeI64 {
                    value: semantic_vocabulary::IntegerValue::Unsigned(ordinal as u128),
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
        }
        let SelectedTerminator::ConditionalBranch {
            instruction,
            when_zero,
            when_nonzero,
        } = &replay.block.terminator
        else {
            return Err(invalid());
        };
        successor(&cases[ordinal], when_zero, slot, replay)?;
        let next = if ordinal + 2 == cases.len() {
            successor(&cases[ordinal + 1], when_nonzero, slot, replay)?;
            None
        } else {
            let expected_origin = selected_instructions::SelectedBlockOrigin::CaseDispatch {
                source: block.id,
                case_ordinal: (ordinal + 1).try_into().map_err(|_| invalid())?,
            };
            let next = replay
                .selected
                .blocks
                .iter()
                .find(|candidate| candidate.origin == expected_origin)
                .ok_or_else(invalid)?;
            if when_nonzero.role != SelectedSuccessorRole::CaseDispatchContinuation
                || when_nonzero.psi_edge != cases[ordinal + 1].edge
                || when_nonzero.source_target != block.id
                || when_nonzero.block != next.id
                || !when_nonzero.fuel.is_empty()
                || !when_nonzero.bindings.is_empty()
                || !when_nonzero.structural_bindings.is_empty()
                || when_nonzero.structural_case.is_some()
            {
                return Err(invalid());
            }
            Some(next)
        };
        if instruction.id.0 as usize != replay.instruction_cursor
            || instruction.kind != SelectedInstructionKind::ConditionalBranchNonZero
            || instruction.constraint != replay.constraints.keys.conditional_branch
            || !instruction.operands.is_empty()
            || instruction.provenance != SelectedInstructionProvenance::default()
            || replay.block_cursor != replay.block.instructions.len()
        {
            return Err(invalid());
        }
        if ordinal == 0 {
            original_cursor = Some(replay.block_cursor);
        } else {
            validate_block_constraints(replay.function, replay.block, replay.selected, catalog)?;
        }
        if let Some(next) = next {
            replay.instruction_cursor = replay
                .instruction_cursor
                .checked_add(1)
                .ok_or_else(invalid)?;
            replay.block = next;
            replay.block_cursor = 0;
        }
    }
    replay.block = original_block;
    replay.block_cursor = original_cursor.ok_or_else(invalid)?;
    Ok(())
}

fn successor(
    expected: &legalized_operations::LegalizedStructuralCaseSuccessor,
    actual: &selected_instructions::SelectedSuccessor,
    slot: LocalStorageSlotId,
    replay: &Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let destination = replay
        .selected
        .blocks
        .iter()
        .find(|candidate| {
            candidate.origin == selected_instructions::SelectedBlockOrigin::Source(expected.target)
        })
        .ok_or_else(invalid)?;
    let retained = actual.structural_case.as_ref().ok_or_else(invalid)?;
    if actual.role != SelectedSuccessorRole::Semantic
        || actual.psi_edge != expected.edge
        || actual.source_target != expected.target
        || actual.block != destination.id
        || actual.fuel != expected.fuel
        || !actual.bindings.is_empty()
        || !actual.structural_bindings.is_empty()
        || retained.slot != slot
        || retained.case != expected.case
        || retained.case_tag != expected.case_tag
        || retained.trivial_affine_discards != expected.trivial_affine_discards
        || retained.payloads.len() != expected.payloads.len()
    {
        return Err(invalid());
    }
    for (actual, expected) in retained.payloads.iter().zip(&expected.payloads) {
        if actual.semantic != *expected {
            return Err(invalid());
        }
        let parameter = replay.selected.virtual_registers.iter().find(|row| {
            matches!(row.origin, VirtualRegisterOrigin::BlockParameter {
                    source_value, block, ..
                } if source_value == expected.parameter.value && block == destination.id)
        });
        match (
            replay.required_values.contains(&expected.parameter.value),
            parameter,
            actual.transport,
        ) {
            (false, None, SelectedCasePayloadTransport::Unused) => {}
            (
                true,
                Some(parameter),
                SelectedCasePayloadTransport::Unmaterialized { parameter: id },
            ) if parameter.id == id
                && parameter.scalar_type == expected.parameter.scalar_type
                && parameter.definition_site == Some(expected.parameter.definition_site) => {}
            _ => return Err(invalid()),
        }
    }
    Ok(())
}

pub(super) fn temporary(
    replay: &mut Replay<'_>,
    place: PlaceId,
    byte_offset: u32,
    observation: bool,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let instruction = SelectedInstructionId(
        replay
            .instruction_cursor
            .try_into()
            .map_err(|_| replay.invalid())?,
    );
    let origin = if observation {
        VirtualRegisterOrigin::StructuralObservation {
            instruction,
            place,
            byte_offset,
        }
    } else {
        VirtualRegisterOrigin::AbiTransport {
            instruction,
            place,
            byte_offset,
        }
    };
    let scalar_type = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, if observation { 32 } else { 64 })
            .map_err(|_| replay.invalid())?,
    );
    let row = replay
        .selected
        .virtual_registers
        .get(replay.register_cursor)
        .ok_or_else(|| replay.invalid())?;
    if row.id.0 as usize != replay.register_cursor
        || row.origin != origin
        || row.scalar_type != scalar_type
        || row.class != replay.class
        || row.definition_site.is_some()
        || row.entry_fixed_view.is_some()
    {
        return Err(replay.invalid());
    }
    replay.register_cursor += 1;
    Ok(row.id)
}

pub(super) fn memory(
    replay: &mut Replay<'_>,
    block: semantic_vocabulary::BlockId,
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    role: SelectedMemoryAccessRole,
) -> Result<(), SelectedInstructionError> {
    replay.transport.memory.push(SelectedMemoryAccess {
        instruction: SelectedInstructionId(
            replay
                .instruction_cursor
                .try_into()
                .map_err(|_| replay.invalid())?,
        ),
        origin: SelectedMemoryAccessOrigin::Block(block),
        place,
        byte_offset,
        byte_count,
        role,
    });
    Ok(())
}
