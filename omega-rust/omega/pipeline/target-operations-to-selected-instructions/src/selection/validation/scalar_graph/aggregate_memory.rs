//! Independently replay exact aggregate fragment accesses without widening storage.
//! Packed expansions own an instruction-local temporary, never a source value.
//! The caller retains the exact place/offset/width access record; the physical
//! constraints keep scratch and early-written outputs disjoint from live inputs.
use super::*;

pub(super) fn load(
    replay: &mut Replay<'_>,
    pointer: VirtualRegisterId,
    output: VirtualRegisterId,
    byte_offset: u32,
    byte_size: u16,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let mut operands = vec![pointer, output];
    let (kind, key) = match byte_size {
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
        _ => {
            let width = selected_instructions::PackedByteWidth::from_byte_size(
                byte_size.try_into().map_err(|_| invalid())?,
            )
            .ok_or_else(invalid)?;
            operands.push(scratch(replay)?);
            (
                SelectedInstructionKind::LoadPacked { byte_offset, width },
                replay.constraints.keys.load_packed,
            )
        }
    };
    replay.check_instruction(
        kind,
        key.ok_or_else(invalid)?,
        &operands,
        &Default::default(),
    )
}

pub(super) fn store(
    replay: &mut Replay<'_>,
    pointer: VirtualRegisterId,
    value: VirtualRegisterId,
    byte_offset: u32,
    byte_size: u16,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let byte_size = u8::try_from(byte_size).map_err(|_| invalid())?;
    let mut operands = vec![pointer, value];
    let (kind, key) =
        if let Some(width) = selected_instructions::PackedByteWidth::from_byte_size(byte_size) {
            operands.push(scratch(replay)?);
            (
                SelectedInstructionKind::StorePacked { byte_offset, width },
                replay.constraints.keys.store_packed,
            )
        } else if matches!(byte_size, 1 | 2 | 4 | 8) {
            (
                SelectedInstructionKind::Store {
                    byte_offset,
                    byte_size,
                },
                replay.constraints.keys.store,
            )
        } else {
            return Err(invalid());
        };
    replay.check_instruction(
        kind,
        key.ok_or_else(invalid)?,
        &operands,
        &Default::default(),
    )
}

fn scratch(replay: &mut Replay<'_>) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let instruction = SelectedInstructionId(
        replay
            .instruction_cursor
            .try_into()
            .map_err(|_| replay.invalid())?,
    );
    let expected_type = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64)
            .map_err(|_| replay.invalid())?,
    );
    let register = replay
        .selected
        .virtual_registers
        .get(replay.register_cursor)
        .ok_or_else(|| replay.invalid())?;
    if register.id.0 as usize != replay.register_cursor
        || register.origin
            != (VirtualRegisterOrigin::InstructionScratch {
                instruction,
                operand: 2,
            })
        || register.scalar_type != expected_type
        || register.class != replay.class
        || register.definition_site.is_some()
        || register.entry_fixed_view.is_some()
    {
        return Err(replay.invalid());
    }
    replay.register_cursor += 1;
    Ok(register.id)
}
